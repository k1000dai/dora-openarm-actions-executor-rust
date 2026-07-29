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

//! Arms configuration: which arm sides are enabled, and how a joined
//! position vector splits into each enabled side's slice.

/// The number of position elements (7 joints + 1 gripper) each arm side
/// contributes to a joined position vector.
///
/// Matches upstream's `n_elements = 8`.
pub const ELEMENTS_PER_ARM: usize = 8;

/// Which arm sides are enabled for this node.
///
/// Mirrors upstream's `arms = args.arms.split(",")` followed by plain list
/// membership checks (`"right" in arms`, `"left" in arms`): a side is
/// enabled only if the comma-separated string contains a token that is
/// exactly `"right"` or `"left"` (no trimming).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arms {
    has_right: bool,
    has_left: bool,
}

impl Arms {
    /// Parses an arms configuration from a comma-separated string, e.g.
    /// `"right,left"`.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let mut has_right = false;
        let mut has_left = false;
        for token in raw.split(',') {
            match token {
                "right" => has_right = true,
                "left" => has_left = true,
                _ => {}
            }
        }
        Self {
            has_right,
            has_left,
        }
    }

    /// Whether the right arm is enabled.
    #[must_use]
    pub fn has_right(&self) -> bool {
        self.has_right
    }

    /// Whether the left arm is enabled.
    #[must_use]
    pub fn has_left(&self) -> bool {
        self.has_left
    }

    /// Splits a joined position vector into the right and left arm's own
    /// slices, according to which sides are enabled.
    ///
    /// Mirrors upstream's offset walk: the right side, if enabled, always
    /// reads the first [`ELEMENTS_PER_ARM`] elements and advances the
    /// offset; the left side then reads the next [`ELEMENTS_PER_ARM`]
    /// elements starting from whatever offset the right side left behind
    /// -- offset `0` if the right side is disabled, even when only the
    /// left side is enabled.
    #[must_use]
    pub fn split_position(&self, position: &[f32]) -> (Option<Vec<f32>>, Option<Vec<f32>>) {
        let mut offset = 0;
        let right = if self.has_right {
            let slice = position[offset..offset + ELEMENTS_PER_ARM].to_vec();
            offset += ELEMENTS_PER_ARM;
            Some(slice)
        } else {
            None
        };
        let left = if self.has_left {
            Some(position[offset..offset + ELEMENTS_PER_ARM].to_vec())
        } else {
            None
        };
        (right, left)
    }
}
