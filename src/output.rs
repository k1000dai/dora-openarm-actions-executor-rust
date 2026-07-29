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

//! Construction of the `move_position_right`/`move_position_left`
//! output arrays from one step's split arm positions.

use std::sync::Arc;

use arrow::array::{ArrayRef, Float32Array, StructArray};
use arrow::datatypes::{DataType, Field, Fields};

/// The output id for the right arm's motor command.
pub const OUTPUT_MOVE_POSITION_RIGHT: &str = "move_position_right";

/// The output id for the left arm's motor command.
pub const OUTPUT_MOVE_POSITION_LEFT: &str = "move_position_left";

fn plain_array(position: &[f32]) -> ArrayRef {
    Arc::new(Float32Array::from(position.to_vec()))
}

fn struct_array(own_position: &[f32], other_position: &[f32]) -> ArrayRef {
    let fields = Fields::from(vec![
        Field::new("new_position", DataType::Float32, false),
        Field::new("other_arm_position", DataType::Float32, false),
    ]);
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Float32Array::from(own_position.to_vec())),
        Arc::new(Float32Array::from(other_position.to_vec())),
    ];
    Arc::new(StructArray::new(fields, columns, None))
}

/// Builds the `move_position_right`/`move_position_left` outputs for one
/// step's split arm positions, in that order.
///
/// Mirrors upstream: an enabled arm with the other arm also enabled
/// sends a `StructArray` of `new_position` (its own position) and
/// `other_arm_position` (the other arm's position); an enabled arm with
/// the other disabled sends its position as a plain array; a disabled
/// arm sends no output at all.
#[must_use]
pub fn build_outputs(right: Option<&[f32]>, left: Option<&[f32]>) -> Vec<(&'static str, ArrayRef)> {
    let mut outputs = Vec::new();
    if let Some(right) = right {
        let array = match left {
            Some(left) => struct_array(right, left),
            None => plain_array(right),
        };
        outputs.push((OUTPUT_MOVE_POSITION_RIGHT, array));
    }
    if let Some(left) = left {
        let array = match right {
            Some(right) => struct_array(left, right),
            None => plain_array(left),
        };
        outputs.push((OUTPUT_MOVE_POSITION_LEFT, array));
    }
    outputs
}
