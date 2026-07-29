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

//! Tustin bilinear-transform biquad low-pass filter, used to smooth
//! upsampled outputs.

/// The default quality factor upstream uses for [`BiquadLowpass`],
/// matching a Butterworth response.
pub const DEFAULT_Q: f64 = 0.707;

/// Biquad low-pass filter for smoothing outputs.
///
/// Coefficients follow the RBJ audio EQ cookbook low-pass formula, using
/// the Tustin (bilinear transform) discretization.
#[derive(Debug, Clone)]
pub struct BiquadLowpass {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    state: Option<FilterState>,
}

#[derive(Debug, Clone)]
struct FilterState {
    x1: Vec<f32>,
    x2: Vec<f32>,
    y1: Vec<f32>,
    y2: Vec<f32>,
}

impl BiquadLowpass {
    /// Creates a low-pass filter for sampling frequency `fs`, cutoff
    /// frequency `fc`, and quality factor `q`.
    #[must_use]
    pub fn new(fs: f64, fc: f64, q: f64) -> Self {
        let w0 = 2.0 * std::f64::consts::PI * fc / fs;
        let cosw0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b0 = ((1.0 - cosw0) / 2.0) / a0;
        let b1 = (1.0 - cosw0) / a0;
        let b2 = b0;
        let a1 = (-2.0 * cosw0) / a0;
        let a2 = (1.0 - alpha) / a0;
        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            state: None,
        }
    }

    /// Resets the filter state as though `initial_x` had been its input
    /// and output for all prior samples.
    pub fn reset_state(&mut self, initial_x: &[f32]) {
        self.state = Some(FilterState {
            x1: initial_x.to_vec(),
            x2: initial_x.to_vec(),
            y1: initial_x.to_vec(),
            y2: initial_x.to_vec(),
        });
    }

    /// Applies one step of the filter to `x`, lazily resetting the state
    /// to `x` first if this is the first call since construction (or
    /// since the state was otherwise unset).
    ///
    /// # Panics
    ///
    /// Never panics in practice: the state is always set, either just
    /// above or by a prior call, before it is read.
    pub fn step(&mut self, x: &[f32]) -> Vec<f32> {
        if self.state.is_none() {
            self.reset_state(x);
        }
        let state = self.state.as_mut().expect("state was just set");

        let y: Vec<f32> = (0..x.len())
            .map(|i| {
                let value = self.b0 * f64::from(x[i])
                    + self.b1 * f64::from(state.x1[i])
                    + self.b2 * f64::from(state.x2[i])
                    - self.a1 * f64::from(state.y1[i])
                    - self.a2 * f64::from(state.y2[i]);
                value as f32
            })
            .collect();

        state.x2 = std::mem::replace(&mut state.x1, x.to_vec());
        state.y2 = std::mem::replace(&mut state.y1, y.clone());

        y
    }
}
