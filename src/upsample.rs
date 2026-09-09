// Copyright 2026 Enactic, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Cubic Hermite spline upsampling of a coarse trajectory chunk.

use std::fmt;

use crate::numeric::arange;

/// An error upsampling a trajectory chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsampleError {
    /// The chunk's row count did not match the upsampler's configured
    /// chunk timestamp count.
    ChunkLengthMismatch {
        /// The row count the upsampler was configured for.
        expected: usize,
        /// The row count the chunk actually had.
        got: usize,
    },
}

impl fmt::Display for UpsampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChunkLengthMismatch { expected, got } => {
                write!(f, "expected chunk length {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for UpsampleError {}

/// Computes the per-column derivative estimate at each of `y`'s rows,
/// given their timestamps `t`.
///
/// Endpoints take the adjacent secant slope. Interior points average
/// the two adjacent secant slopes, except where they change sign (a
/// local extremum), where the derivative is set to zero to avoid
/// overshoot -- a monotonicity-preserving adjustment.
#[must_use]
pub fn compute_slopes(t: &[f64], y: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let n = y.len();
    let pos_shape = y[0].len();

    let secants: Vec<Vec<f32>> = (0..n - 1)
        .map(|i| {
            let dt = (t[i + 1] - t[i]) as f32;
            (0..pos_shape)
                .map(|col| (y[i + 1][col] - y[i][col]) / dt)
                .collect()
        })
        .collect();

    let mut slopes = vec![vec![0.0f32; pos_shape]; n];
    slopes[0].clone_from(&secants[0]);
    slopes[n - 1].clone_from(&secants[n - 2]);
    for i in 1..n - 1 {
        for col in 0..pos_shape {
            let prev = secants[i - 1][col];
            let next = secants[i][col];
            slopes[i][col] = if prev * next <= 0.0 {
                0.0
            } else {
                prev.midpoint(next)
            };
        }
    }
    slopes
}

/// Upsamples coarse trajectory chunks using cubic Hermite spline
/// interpolation.
#[derive(Debug, Clone)]
pub struct HermiteUpsampler {
    t_chunk: Vec<f64>,
}

impl HermiteUpsampler {
    /// Creates an upsampler for chunks arriving at `chunk_hz`, spanning
    /// `horizon_sec` seconds.
    #[must_use]
    pub fn new(chunk_hz: f64, horizon_sec: f64) -> Self {
        let dt_chunk = 1.0 / chunk_hz;
        let t_chunk = arange(0.0, horizon_sec + 1e-12, dt_chunk);
        Self { t_chunk }
    }

    /// The number of chunk rows this upsampler expects.
    #[must_use]
    pub fn expected_chunk_len(&self) -> usize {
        self.t_chunk.len()
    }

    /// Finds the index `idx` such that `t_chunk[idx] <= v < t_chunk[idx + 1]`,
    /// clamped so `idx` and `idx + 1` both stay in bounds.
    ///
    /// Mirrors upstream's `np.searchsorted(t_chunk, t_eval, side="right") - 1`
    /// followed by `np.clip(idx, 0, len(t_chunk) - 2)`.
    fn segment_index(&self, v: f64) -> usize {
        let count_le = self.t_chunk.iter().filter(|&&t| t <= v).count();
        let idx = count_le.saturating_sub(1);
        idx.min(self.t_chunk.len() - 2)
    }

    /// Upsamples `y_chunk` to the evaluation timestamps in `t_eval`.
    ///
    /// # Errors
    ///
    /// Returns [`UpsampleError::ChunkLengthMismatch`] if `y_chunk`'s row
    /// count does not match [`Self::expected_chunk_len`].
    pub fn upsample(
        &self,
        y_chunk: &[Vec<f32>],
        t_eval: &[f64],
    ) -> Result<Vec<Vec<f32>>, UpsampleError> {
        if y_chunk.len() != self.t_chunk.len() {
            return Err(UpsampleError::ChunkLengthMismatch {
                expected: self.t_chunk.len(),
                got: y_chunk.len(),
            });
        }

        let slopes = compute_slopes(&self.t_chunk, y_chunk);
        let pos_shape = y_chunk[0].len();

        let result = t_eval
            .iter()
            .map(|&v| {
                let idx = self.segment_index(v);
                let t0 = self.t_chunk[idx];
                let t1 = self.t_chunk[idx + 1];
                let h = t1 - t0;
                let u = (v - t0) / h;

                let h00 = 2.0 * u.powi(3) - 3.0 * u.powi(2) + 1.0;
                let h10 = (u.powi(3) - 2.0 * u.powi(2) + u) * h;
                let h01 = -2.0 * u.powi(3) + 3.0 * u.powi(2);
                let h11 = (u.powi(3) - u.powi(2)) * h;

                (0..pos_shape)
                    .map(|col| {
                        let y0 = f64::from(y_chunk[idx][col]);
                        let y1 = f64::from(y_chunk[idx + 1][col]);
                        let m0 = f64::from(slopes[idx][col]);
                        let m1 = f64::from(slopes[idx + 1][col]);
                        (h00 * y0 + h10 * m0 + h01 * y1 + h11 * m1) as f32
                    })
                    .collect()
            })
            .collect();

        Ok(result)
    }
}
