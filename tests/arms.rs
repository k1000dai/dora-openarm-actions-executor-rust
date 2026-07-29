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

//! Behavior tests for arms configuration parsing and position splitting.

use dora_openarm_actions_executor_rust::Arms;

#[test]
fn parses_right_and_left() {
    let arms = Arms::parse("right,left");
    assert!(arms.has_right());
    assert!(arms.has_left());
}

#[test]
fn parses_right_only() {
    let arms = Arms::parse("right");
    assert!(arms.has_right());
    assert!(!arms.has_left());
}

#[test]
fn parses_left_only() {
    let arms = Arms::parse("left");
    assert!(!arms.has_right());
    assert!(arms.has_left());
}

#[test]
fn parses_left_and_right_in_reversed_order() {
    let arms = Arms::parse("left,right");
    assert!(arms.has_right());
    assert!(arms.has_left());
}

#[test]
fn unrecognized_token_enables_neither_arm() {
    // Mirrors upstream's plain list-membership check: an arms string that
    // names neither "right" nor "left" silently enables no arm at all.
    let arms = Arms::parse("both");
    assert!(!arms.has_right());
    assert!(!arms.has_left());
}

#[test]
fn membership_check_does_not_trim_whitespace() {
    // Matches Python's `"right" in arms` on the raw split(",") list: a
    // token with surrounding whitespace does not match "right" exactly.
    let arms = Arms::parse("right ,left");
    assert!(!arms.has_right());
    assert!(arms.has_left());
}

#[test]
fn split_position_with_both_arms_takes_first_then_second_eight() {
    let arms = Arms::parse("right,left");
    let position: Vec<f32> = (0..16).map(|value| value as f32).collect();
    let (right, left) = arms.split_position(&position);
    assert_eq!(
        right.as_deref(),
        Some(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0][..])
    );
    assert_eq!(
        left.as_deref(),
        Some(&[8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0][..])
    );
}

#[test]
fn split_position_with_right_only_leaves_left_none() {
    let arms = Arms::parse("right");
    let position: Vec<f32> = (0..8).map(|value| value as f32).collect();
    let (right, left) = arms.split_position(&position);
    assert_eq!(right.as_deref(), Some(&position[..]));
    assert_eq!(left, None);
}

#[test]
fn split_position_with_left_only_reads_from_offset_zero() {
    // Mirrors upstream: the offset only advances past the right arm's
    // slice when "right" is enabled, so a left-only configuration reads
    // the left arm's 8 elements starting at offset 0, not offset 8.
    let arms = Arms::parse("left");
    let position: Vec<f32> = (0..8).map(|value| value as f32).collect();
    let (right, left) = arms.split_position(&position);
    assert_eq!(right, None);
    assert_eq!(left.as_deref(), Some(&position[..]));
}

#[test]
fn split_position_with_neither_arm_returns_none_for_both() {
    let arms = Arms::parse("both");
    let position: Vec<f32> = (0..16).map(|value| value as f32).collect();
    let (right, left) = arms.split_position(&position);
    assert_eq!(right, None);
    assert_eq!(left, None);
}
