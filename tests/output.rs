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

//! Behavior tests for per-arm `move_position_*` output construction.

use arrow::array::{Float32Array, StructArray};
use dora_openarm_actions_executor_rust::{
    OUTPUT_MOVE_POSITION_LEFT, OUTPUT_MOVE_POSITION_RIGHT, build_outputs,
};

#[test]
fn only_right_arm_sends_a_single_plain_array() {
    let right = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let outputs = build_outputs(Some(&right), None);
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].0, OUTPUT_MOVE_POSITION_RIGHT);
    let array = outputs[0]
        .1
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("plain Float32Array when only one arm is enabled");
    assert_eq!(array.values(), right.as_slice());
}

#[test]
fn only_left_arm_sends_a_single_plain_array() {
    let left = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
    let outputs = build_outputs(None, Some(&left));
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].0, OUTPUT_MOVE_POSITION_LEFT);
    let array = outputs[0]
        .1
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("plain Float32Array when only one arm is enabled");
    assert_eq!(array.values(), left.as_slice());
}

#[test]
fn neither_arm_sends_no_outputs() {
    let outputs = build_outputs(None, None);
    assert!(outputs.is_empty());
}

#[test]
fn both_arms_send_struct_arrays_in_right_then_left_order() {
    let right = vec![1.0; 8];
    let left = vec![2.0; 8];
    let outputs = build_outputs(Some(&right), Some(&left));

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].0, OUTPUT_MOVE_POSITION_RIGHT);
    assert_eq!(outputs[1].0, OUTPUT_MOVE_POSITION_LEFT);

    let right_struct = outputs[0]
        .1
        .as_any()
        .downcast_ref::<StructArray>()
        .expect("StructArray when both arms are enabled");
    let new_position = right_struct
        .column_by_name("new_position")
        .expect("new_position field")
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("Float32Array");
    let other_arm_position = right_struct
        .column_by_name("other_arm_position")
        .expect("other_arm_position field")
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("Float32Array");
    assert_eq!(new_position.values(), right.as_slice());
    assert_eq!(other_arm_position.values(), left.as_slice());

    let left_struct = outputs[1]
        .1
        .as_any()
        .downcast_ref::<StructArray>()
        .expect("StructArray when both arms are enabled");
    let new_position = left_struct
        .column_by_name("new_position")
        .expect("new_position field")
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("Float32Array");
    let other_arm_position = left_struct
        .column_by_name("other_arm_position")
        .expect("other_arm_position field")
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("Float32Array");
    assert_eq!(new_position.values(), left.as_slice());
    assert_eq!(other_arm_position.values(), right.as_slice());
}
