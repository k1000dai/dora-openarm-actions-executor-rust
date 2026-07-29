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

//! Behavior tests for the cubic Hermite spline upsampler.

use dora_openarm_actions_executor_rust::{HermiteUpsampler, UpsampleError, compute_slopes};

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn slopes_at_a_monotonic_interior_point_average_both_secants() {
    let t = vec![0.0, 1.0, 2.0];
    let y = vec![vec![0.0], vec![1.0], vec![3.0]];
    let slopes = compute_slopes(&t, &y);
    assert_close(slopes[0][0], 1.0);
    assert_close(slopes[1][0], 1.5);
    assert_close(slopes[2][0], 2.0);
}

#[test]
fn slopes_at_a_local_extremum_are_zeroed() {
    let t = vec![0.0, 1.0, 2.0];
    let y = vec![vec![0.0], vec![5.0], vec![0.0]];
    let slopes = compute_slopes(&t, &y);
    assert_close(slopes[0][0], 5.0);
    assert_close(slopes[1][0], 0.0);
    assert_close(slopes[2][0], -5.0);
}

#[test]
fn slopes_are_computed_independently_per_joint_column() {
    let t = vec![0.0, 1.0, 2.0];
    let y = vec![vec![0.0, 0.0], vec![1.0, 5.0], vec![3.0, 0.0]];
    let slopes = compute_slopes(&t, &y);
    // Column 0 is the monotonic case (averaged slope), column 1 is the
    // local-extremum case (zeroed slope), independently of each other.
    assert_close(slopes[1][0], 1.5);
    assert_close(slopes[1][1], 0.0);
}

#[test]
fn two_point_chunk_upsamples_to_exact_linear_interpolation() {
    // With only one secant, both endpoint derivatives equal that secant,
    // which makes the cubic Hermite segment reduce to a straight line.
    let upsampler = HermiteUpsampler::new(1.0, 1.0);
    let y_chunk = vec![vec![0.0], vec![10.0]];
    let t_eval = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    let result = upsampler.upsample(&y_chunk, &t_eval).unwrap();
    let expected = [0.0, 2.5, 5.0, 7.5, 10.0];
    for (row, expected_value) in result.iter().zip(expected.iter()) {
        assert_close(row[0], *expected_value);
    }
}

#[test]
fn evaluating_exactly_at_a_chunk_timestamp_returns_that_sample() {
    let upsampler = HermiteUpsampler::new(1.0, 2.0);
    let y_chunk = vec![vec![0.0], vec![5.0], vec![0.0]];
    let result = upsampler.upsample(&y_chunk, &[1.0]).unwrap();
    assert_close(result[0][0], 5.0);
}

#[test]
fn interpolates_a_zeroed_extremum_slope_at_the_segment_midpoint() {
    // Hand-derived: h00=0.5, h10=0.125, h01=0.5, h11=-0.125 at u=0.5, h=1.
    // y0=0, y1=5, m0=5 (endpoint secant), m1=0 (zeroed extremum slope).
    // result = 0.5*0 + 0.125*5 + 0.5*5 + (-0.125)*0 = 3.125.
    let upsampler = HermiteUpsampler::new(1.0, 2.0);
    let y_chunk = vec![vec![0.0], vec![5.0], vec![0.0]];
    let result = upsampler.upsample(&y_chunk, &[0.5]).unwrap();
    assert_close(result[0][0], 3.125);
}

#[test]
fn interpolates_symmetrically_past_the_zeroed_extremum() {
    // Same hand-derived basis functions (u=0.5, h=1), mirrored: y0=5,
    // y1=0, m0=0, m1=-5.
    // result = 0.5*5 + 0.125*0 + 0.5*0 + (-0.125)*(-5) = 3.125.
    let upsampler = HermiteUpsampler::new(1.0, 2.0);
    let y_chunk = vec![vec![0.0], vec![5.0], vec![0.0]];
    let result = upsampler.upsample(&y_chunk, &[1.5]).unwrap();
    assert_close(result[0][0], 3.125);
}

#[test]
fn chunk_length_mismatch_is_an_error() {
    let upsampler = HermiteUpsampler::new(1.0, 2.0);
    let y_chunk = vec![vec![0.0], vec![5.0]];
    let error = upsampler.upsample(&y_chunk, &[0.5]).unwrap_err();
    assert_eq!(
        error,
        UpsampleError::ChunkLengthMismatch {
            expected: 3,
            got: 2,
        }
    );
}
