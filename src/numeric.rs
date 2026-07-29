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

//! A `numpy.arange`-equivalent helper for building evenly spaced
//! timestamp sequences.

/// Builds the half-open sequence `start, start + step, ...` up to (but
/// excluding) `stop`, matching `numpy.arange(start, stop, step)`'s length
/// convention of `ceil((stop - start) / step)` elements.
///
/// Only used with `step > 0.0` and `stop >= start` by this crate's
/// callers.
#[must_use]
#[allow(clippy::similar_names)] // start/stop/step mirror numpy.arange's own parameter names.
pub fn arange(start: f64, stop: f64, step: f64) -> Vec<f64> {
    let len = ((stop - start) / step).ceil();
    let len = if len > 0.0 { len as usize } else { 0 };
    (0..len).map(|i| start + (i as f64) * step).collect()
}
