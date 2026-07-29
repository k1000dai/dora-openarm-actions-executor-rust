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

//! Crossfade blending of a canceled trajectory with the next chunk.

use std::fmt;

/// An error blending a canceled trajectory with the next chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendError {
    /// The next chunk has fewer rows than the canceled trajectory being
    /// blended in, so the two cannot be combined row-for-row.
    InsufficientNextPositions {
        /// Number of rows in the canceled trajectory.
        canceled_len: usize,
        /// Number of rows in the next chunk.
        next_len: usize,
    },
}

impl fmt::Display for BlendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientNextPositions {
                canceled_len,
                next_len,
            } => write!(
                f,
                "cannot blend {canceled_len} canceled rows with only {next_len} next rows"
            ),
        }
    }
}

impl std::error::Error for BlendError {}

/// Linearly spaced weights from `1.0` down to `0.0`, matching numpy's
/// `np.linspace(1, 0, n)`.
fn descending_weights(n: usize) -> Vec<f32> {
    match n {
        0 => Vec::new(),
        1 => vec![1.0],
        _ => {
            let denom = (n - 1) as f32;
            (0..n).map(|i| 1.0 - (i as f32) / denom).collect()
        }
    }
}

/// Crossfades a canceled trajectory into the start of the next chunk.
///
/// Mirrors upstream's `blend`: `canceled_positions` fades out linearly
/// (weight `1.0` at its first row down to `0.0` at its last) while the
/// overlapping prefix of `next_positions` fades in. Returns the blended
/// rows and their count, `n`, so the caller can append
/// `next_positions[n..]` to complete the chunk.
///
/// # Errors
///
/// Returns [`BlendError::InsufficientNextPositions`] if `next_positions`
/// has fewer rows than `canceled_positions`.
pub fn blend(
    canceled_positions: &[Vec<f32>],
    next_positions: &[Vec<f32>],
) -> Result<(Vec<Vec<f32>>, usize), BlendError> {
    let n = canceled_positions.len();
    if next_positions.len() < n {
        return Err(BlendError::InsufficientNextPositions {
            canceled_len: n,
            next_len: next_positions.len(),
        });
    }

    let weights = descending_weights(n);
    let blended = canceled_positions
        .iter()
        .zip(next_positions.iter())
        .zip(weights.iter())
        .map(|((canceled_row, next_row), &weight)| {
            canceled_row
                .iter()
                .zip(next_row.iter())
                .map(|(&canceled_value, &next_value)| {
                    canceled_value * weight + next_value * (1.0 - weight)
                })
                .collect()
        })
        .collect();

    Ok((blended, n))
}
