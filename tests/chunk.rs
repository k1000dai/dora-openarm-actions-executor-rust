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

//! Behavior tests for parsing the `actions` input's position rows out of
//! its Arrow `List<Float32>` value array.

use std::sync::Arc;

use arrow::array::{ArrayRef, Float32Array, Int32Array, ListArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use dora_openarm_actions_executor_rust::{ChunkError, parse_positions};

fn list_array_of(rows: &[Vec<f32>]) -> ListArray {
    let offsets = OffsetBuffer::from_lengths(rows.iter().map(Vec::len));
    let values: Vec<f32> = rows.iter().flatten().copied().collect();
    let field = Arc::new(Field::new("item", DataType::Float32, true));
    ListArray::new(field, offsets, Arc::new(Float32Array::from(values)), None)
}

#[test]
fn parses_uniform_rows() {
    let rows = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
    let array = list_array_of(&rows);
    let parsed = parse_positions(&array).expect("valid chunk");
    assert_eq!(parsed, rows);
}

#[test]
fn empty_chunk_is_an_error() {
    let array = list_array_of(&[]);
    let error = parse_positions(&array).unwrap_err();
    assert_eq!(error, ChunkError::EmptyChunk);
}

#[test]
fn ragged_row_is_an_error() {
    let rows = vec![vec![1.0, 2.0], vec![3.0]];
    let array = list_array_of(&rows);
    let error = parse_positions(&array).unwrap_err();
    assert_eq!(
        error,
        ChunkError::RaggedRow {
            index: 1,
            expected: 2,
            got: 1,
        }
    );
}

#[test]
fn non_float32_child_is_an_error() {
    let offsets = OffsetBuffer::from_lengths([2, 2]);
    let field = Arc::new(Field::new("item", DataType::Int32, true));
    let values: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3, 4]));
    let array = ListArray::new(field, offsets, values, None);
    let error = parse_positions(&array).unwrap_err();
    assert_eq!(error, ChunkError::RowNotFloat32 { index: 0 });
}
