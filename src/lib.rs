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

//! Pure logic of a dora-rs node that executes timestamped actions.
//!
//! This library knows nothing about dora-rs. It answers the questions
//! that decide what to publish for one action chunk -- how to split
//! configured arms out of a joined position vector, how to blend a
//! canceled trajectory with the next chunk, how to upsample a coarse
//! chunk with cubic Hermite interpolation, how to low-pass filter the
//! upsampled stream, how to parse the incoming Arrow action chunk, and
//! how the whole per-chunk schedule advances and reacts to new input or
//! termination -- so those answers can be tested without a running
//! dataflow. The dora-rs event loop and wall-clock timing live in the
//! upstream-compatible `dora-openarm-actions-executor` binary
//! (`src/main.rs`).

pub mod arms;
pub mod blend;
pub mod chunk;
pub mod cli;
pub mod executor;
pub mod lowpass;
pub mod numeric;
pub mod output;
pub mod upsample;

pub use arms::{Arms, ELEMENTS_PER_ARM};
pub use blend::{BlendError, blend};
pub use chunk::{ActionChunk, ChunkError, parse_positions};
pub use cli::{CliError, CliOptions, parse_args};
pub use executor::{
    ChunkOutcome, Environment, PendingSignal, RunChunkError, StepOutput, TrajectoryExecutor,
};
pub use lowpass::{BiquadLowpass, DEFAULT_Q};
pub use numeric::arange;
pub use output::{OUTPUT_MOVE_POSITION_LEFT, OUTPUT_MOVE_POSITION_RIGHT, build_outputs};
pub use upsample::{HermiteUpsampler, UpsampleError, compute_slopes};
