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

//! Parsing of the `actions` input's position rows and metadata into an
//! [`ActionChunk`].

use std::fmt;

use arrow::array::{Array, Float32Array, ListArray};

/// An error parsing the `actions` input's position rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkError {
    /// The chunk had no rows.
    EmptyChunk,
    /// A row's values were not a `Float32` array.
    RowNotFloat32 {
        /// Index of the offending row.
        index: usize,
    },
    /// A row's length did not match the first row's length.
    RaggedRow {
        /// Index of the offending row.
        index: usize,
        /// The length established by the first row.
        expected: usize,
        /// The offending row's actual length.
        got: usize,
    },
}

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyChunk => write!(f, "action chunk has no rows"),
            Self::RowNotFloat32 { index } => {
                write!(f, "action chunk row {index} is not a Float32 array")
            }
            Self::RaggedRow {
                index,
                expected,
                got,
            } => write!(
                f,
                "action chunk row {index} has length {got}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ChunkError {}

/// Parses the `actions` input's `List<Float32>` value array into one
/// `Vec<f32>` row per action step.
///
/// Mirrors upstream's `event["value"].values.to_numpy().reshape(n_positions, pos_shape)`.
///
/// # Errors
///
/// Returns [`ChunkError::EmptyChunk`] if the chunk has no rows,
/// [`ChunkError::RowNotFloat32`] if a row's values are not `Float32`, and
/// [`ChunkError::RaggedRow`] if a row's length does not match the first
/// row's length.
pub fn parse_positions(value: &ListArray) -> Result<Vec<Vec<f32>>, ChunkError> {
    if value.is_empty() {
        return Err(ChunkError::EmptyChunk);
    }

    let mut expected: Option<usize> = None;
    let mut positions = Vec::with_capacity(value.len());
    for index in 0..value.len() {
        let row = value.value(index);
        let row = row
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or(ChunkError::RowNotFloat32 { index })?;
        let len = row.len();
        match expected {
            None => expected = Some(len),
            Some(expected_len) if expected_len != len => {
                return Err(ChunkError::RaggedRow {
                    index,
                    expected: expected_len,
                    got: len,
                });
            }
            Some(_) => {}
        }
        positions.push(row.values().to_vec());
    }
    Ok(positions)
}

/// One action chunk: the raw position rows and the metadata that governs
/// how they are scheduled.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionChunk {
    /// The parsed position rows, one per action step.
    pub positions: Vec<Vec<f32>>,
    /// The interval, in nanoseconds, between successive rows.
    pub interval_ns: i64,
    /// The low-pass filter cutoff frequency, in Hz.
    pub cutoff_hz: f64,
    /// Whether this chunk starts a new episode.
    pub reset: bool,
}
