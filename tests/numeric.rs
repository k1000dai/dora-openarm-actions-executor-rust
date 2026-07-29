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

//! Behavior tests for the `numpy.arange`-equivalent helper.

use dora_openarm_actions_executor_rust::arange;

#[test]
fn basic_range_matches_numpy_arange() {
    let values = arange(0.0, 1.0, 0.25);
    assert_eq!(values, vec![0.0, 0.25, 0.5, 0.75]);
}

#[test]
fn empty_range_when_stop_equals_start() {
    assert!(arange(0.0, 0.0, 1.0).is_empty());
}

#[test]
fn reconstructs_exactly_n_positions_chunk_timestamps() {
    // Mirrors upstream's `t_chunk = np.arange(0.0, horizon_sec + 1e-12, dt_chunk)`,
    // where `horizon_sec = (n_positions - 1) / chunk_hz` and
    // `dt_chunk = 1 / chunk_hz`: the epsilon must be just large enough
    // that floating-point rounding never drops the final point, for
    // every chunk size and rate this node is configured with.
    for &(n_positions, chunk_hz) in &[(2usize, 30.0), (13, 50.0), (100, 250.0), (7, 15.5)] {
        let chunk_hz: f64 = chunk_hz;
        let dt_chunk = 1.0 / chunk_hz;
        let horizon_sec = (n_positions - 1) as f64 / chunk_hz;
        let t_chunk = arange(0.0, horizon_sec + 1e-12, dt_chunk);
        assert_eq!(
            t_chunk.len(),
            n_positions,
            "n_positions={n_positions}, chunk_hz={chunk_hz}"
        );
        assert!((t_chunk[0] - 0.0).abs() < 1e-9);
        assert!((t_chunk[n_positions - 1] - horizon_sec).abs() < 1e-6);
    }
}

#[test]
fn reconstructs_target_evaluation_timestamps() {
    // Mirrors upstream's `t_eval = np.arange(0.0, horizon_sec + 1e-9, target_interval_s)`.
    let horizon_sec = 0.24;
    let target_interval_s = 1.0 / 250.0;
    let t_eval = arange(0.0, horizon_sec + 1e-9, target_interval_s);
    assert_eq!(t_eval.len(), 61);
    assert!((t_eval[0] - 0.0).abs() < 1e-9);
    assert!((t_eval[60] - horizon_sec).abs() < 1e-6);
}
