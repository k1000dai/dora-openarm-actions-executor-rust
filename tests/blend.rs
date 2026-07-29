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

//! Behavior tests for crossfade blending of a canceled trajectory with
//! the next incoming chunk.

use dora_openarm_actions_executor_rust::{BlendError, blend};

#[test]
fn empty_canceled_positions_blend_to_nothing() {
    let (blended, n) = blend(&[], &[vec![1.0], vec![2.0]]).unwrap();
    assert!(blended.is_empty());
    assert_eq!(n, 0);
}

#[test]
fn single_canceled_row_is_taken_verbatim() {
    // linspace(1, 0, 1) is [1.0]: full weight on the canceled row, none on
    // the overlapping next row.
    let canceled = vec![vec![5.0, 6.0]];
    let next = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
    let (blended, n) = blend(&canceled, &next).unwrap();
    assert_eq!(blended, vec![vec![5.0, 6.0]]);
    assert_eq!(n, 1);
}

#[test]
fn two_canceled_rows_crossfade_linearly() {
    // linspace(1, 0, 2) is [1.0, 0.0]: row 0 is all canceled, row 1 is
    // all next.
    let canceled = vec![vec![0.0], vec![10.0]];
    let next = vec![vec![100.0], vec![200.0], vec![300.0]];
    let (blended, n) = blend(&canceled, &next).unwrap();
    assert_eq!(blended, vec![vec![0.0], vec![200.0]]);
    assert_eq!(n, 2);
}

#[test]
fn three_canceled_rows_crossfade_at_the_midpoint() {
    // linspace(1, 0, 3) is [1.0, 0.5, 0.0].
    let canceled = vec![vec![0.0], vec![0.0], vec![0.0]];
    let next = vec![vec![10.0], vec![10.0], vec![10.0], vec![10.0]];
    let (blended, n) = blend(&canceled, &next).unwrap();
    assert_eq!(blended, vec![vec![0.0], vec![5.0], vec![10.0]]);
    assert_eq!(n, 3);
}

#[test]
fn more_canceled_rows_than_next_rows_is_an_error() {
    // Mirrors upstream's numpy broadcast error: `next_positions[:n]` has
    // fewer rows than `canceled_positions` once the new chunk is shorter
    // than the canceled tail, and elementwise multiplication cannot
    // broadcast a shape mismatch.
    let canceled = vec![vec![0.0], vec![0.0], vec![0.0]];
    let next = vec![vec![1.0]];
    let error = blend(&canceled, &next).unwrap_err();
    assert_eq!(
        error,
        BlendError::InsufficientNextPositions {
            canceled_len: 3,
            next_len: 1,
        }
    );
}
