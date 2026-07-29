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

//! Behavior tests for the Tustin bilinear-transform biquad low-pass
//! filter.
//!
//! Expected numeric values were derived independently from the RBJ
//! cookbook biquad formula (the same formula upstream implements) using
//! plain `math.cos`/`math.sin`, not from the Rust implementation itself.

use dora_openarm_actions_executor_rust::BiquadLowpass;

const DEFAULT_Q: f64 = 0.707;

fn assert_close(actual: f32, expected: f64) {
    assert!(
        (f64::from(actual) - expected).abs() < 1e-6,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn step_response_after_reset_matches_the_rbj_biquad_formula() {
    let mut filter = BiquadLowpass::new(100.0, 10.0, DEFAULT_Q);
    filter.reset_state(&[0.0]);
    let expected = [
        0.067_452_282_817_190_11,
        0.279_450_073_978_175_34,
        0.561_360_769_761_952_3,
        0.796_065_164_625_337_3,
        0.947_960_291_423_087_1,
    ];
    for expected_value in expected {
        let y = filter.step(&[1.0]);
        assert_close(y[0], expected_value);
    }
}

#[test]
fn converges_to_a_sustained_step_input() {
    let mut filter = BiquadLowpass::new(100.0, 10.0, DEFAULT_Q);
    filter.reset_state(&[0.0]);
    let mut y = vec![0.0];
    for _ in 0..500 {
        y = filter.step(&[1.0]);
    }
    assert_close(y[0], 1.0);
}

#[test]
fn first_step_without_an_explicit_reset_lazily_initializes_to_the_input() {
    // Matches upstream: `if self.x1 is None: self.reset_state(x)` runs
    // before computing the first output, and the filter's DC gain is
    // exactly 1, so the very first sample passes through unchanged.
    let mut filter = BiquadLowpass::new(100.0, 10.0, DEFAULT_Q);
    let y = filter.step(&[3.0]);
    assert_close(y[0], 3.0);
    let y = filter.step(&[3.0]);
    assert_close(y[0], 3.0);
}

#[test]
fn reset_state_mid_stream_snaps_immediately_to_the_new_value() {
    let mut filter = BiquadLowpass::new(100.0, 10.0, DEFAULT_Q);
    for _ in 0..5 {
        filter.step(&[1.0]);
    }
    filter.reset_state(&[5.0]);
    let y = filter.step(&[5.0]);
    assert_close(y[0], 5.0);
}

#[test]
fn columns_are_filtered_independently() {
    let mut filter = BiquadLowpass::new(100.0, 10.0, DEFAULT_Q);
    filter.reset_state(&[0.0, 0.0]);
    let y = filter.step(&[1.0, -1.0]);
    assert_close(y[0], 0.067_452_282_817_190_11);
    assert_close(y[1], -0.067_452_282_817_190_11);
}
