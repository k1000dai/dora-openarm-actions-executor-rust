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

use arrow::array::{ArrayRef, Float32Array, ListArray, StructArray};
use arrow::buffer::{OffsetBuffer, ScalarBuffer};
use arrow::datatypes::{DataType, Field, Fields};

/// The output id for the right arm's motor command.
pub const OUTPUT_MOVE_POSITION_RIGHT: &str = "move_position_right";

/// The output id for the left arm's motor command.
pub const OUTPUT_MOVE_POSITION_LEFT: &str = "move_position_left";

fn qpos_array(position: &[f32]) -> ArrayRef {
    let item_field = Arc::new(Field::new("item", DataType::Float32, true));
    let values: ArrayRef = Arc::new(Float32Array::from(position.to_vec()));
    let length = i32::try_from(position.len()).expect("a joint position row fits in i32");
    let offsets = OffsetBuffer::new(ScalarBuffer::from(vec![0_i32, length]));
    let list: ArrayRef = Arc::new(ListArray::new(item_field, offsets, values, None));
    let fields = Fields::from(vec![Field::new("qpos", list.data_type().clone(), true)]);
    Arc::new(
        StructArray::try_new(fields, vec![list], None)
            .expect("single-field qpos struct is always valid"),
    )
}

/// Builds the `move_position_right`/`move_position_left` outputs for one
/// step's split arm positions, in that order.
///
/// Mirrors upstream: every enabled arm sends a length-one `StructArray`
/// with one `qpos: List<Float32>` field; a disabled arm sends no output.
#[must_use]
pub fn build_outputs(right: Option<&[f32]>, left: Option<&[f32]>) -> Vec<(&'static str, ArrayRef)> {
    let mut outputs = Vec::new();
    if let Some(right) = right {
        outputs.push((OUTPUT_MOVE_POSITION_RIGHT, qpos_array(right)));
    }
    if let Some(left) = left {
        outputs.push((OUTPUT_MOVE_POSITION_LEFT, qpos_array(left)));
    }
    outputs
}
