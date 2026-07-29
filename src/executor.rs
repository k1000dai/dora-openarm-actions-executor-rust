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

//! The stateful per-chunk trajectory scheduler.
//!
//! [`TrajectoryExecutor::run_chunk`] is the pure heart of this node: it
//! decides, for one action chunk, whether to (re-)initialize the
//! upsampler and filter, whether to reset carried-over state, how to
//! blend in a canceled trajectory, and how the resulting steps are
//! timed and can be interrupted -- all driven through the small
//! [`Environment`] trait, so the whole schedule is testable with a fake
//! clock and a scripted signal sequence instead of a real dora runtime.

use std::fmt;

use crate::blend::{BlendError, blend};
use crate::chunk::ActionChunk;
use crate::lowpass::{BiquadLowpass, DEFAULT_Q};
use crate::numeric::arange;
use crate::upsample::{HermiteUpsampler, UpsampleError};

/// A signal observed while stepping through a chunk's trajectory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSignal {
    /// Nothing new has arrived; keep executing the current step.
    None,
    /// A new action chunk has already arrived; cancel the current
    /// trajectory and hand control back to the caller so it can fetch
    /// that chunk next.
    NewChunk,
    /// The node should stop entirely, discarding the rest of the current
    /// trajectory.
    Terminate,
}

/// The real-time primitives [`TrajectoryExecutor::run_chunk`] needs:
/// reading the clock, sleeping, and checking whether a new event has
/// already arrived. The dora adapter implements this against the real
/// clock and event stream; tests implement it against a fake, virtual
/// one.
pub trait Environment {
    /// Returns the current time, in nanoseconds.
    fn now_ns(&mut self) -> i64;
    /// Sleeps for `duration_ns` nanoseconds.
    fn sleep_ns(&mut self, duration_ns: i64);
    /// Checks, without blocking, whether a new event has already
    /// arrived or the node should terminate.
    fn poll(&mut self) -> PendingSignal;
}

/// One emitted step: a joined position vector, ready to be split across
/// arms, and the timestamp it was sent at.
#[derive(Debug, Clone, PartialEq)]
pub struct StepOutput {
    /// The joined position vector for this step.
    pub position: Vec<f32>,
    /// The timestamp, in nanoseconds, this step was sent at.
    pub timestamp_ns: i64,
}

/// How a call to [`TrajectoryExecutor::run_chunk`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkOutcome {
    /// Every step in the chunk was sent.
    Completed,
    /// A new chunk had already arrived; the caller should fetch it next.
    CancelledForNextChunk,
    /// The node should terminate.
    Terminated,
}

/// An error running one chunk's trajectory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunChunkError {
    /// Blending the canceled trajectory into the next chunk failed.
    Blend(BlendError),
    /// Upsampling the chunk failed.
    Upsample(UpsampleError),
}

impl fmt::Display for RunChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blend(error) => write!(f, "{error}"),
            Self::Upsample(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RunChunkError {}

impl From<BlendError> for RunChunkError {
    fn from(error: BlendError) -> Self {
        Self::Blend(error)
    }
}

impl From<UpsampleError> for RunChunkError {
    fn from(error: UpsampleError) -> Self {
        Self::Upsample(error)
    }
}

/// Per-run state carried across chunks: the lazily initialized upsampler
/// and filter, and any trajectory canceled by an already-arrived chunk.
#[derive(Debug)]
pub struct TrajectoryExecutor {
    use_upsample: bool,
    use_filter: bool,
    control_hz: f64,
    canceled_positions: Option<Vec<Vec<f32>>>,
    upsampler: Option<HermiteUpsampler>,
    lowpass: Option<BiquadLowpass>,
    dynamic_chunk_hz: Option<f64>,
    target_interval_ns: Option<i64>,
    target_interval_s: Option<f64>,
    t_eval: Option<Vec<f64>>,
}

impl TrajectoryExecutor {
    /// Creates an executor for the given upsample/filter/control-rate
    /// configuration.
    ///
    /// Mirrors upstream: a `filter` request without `upsample` has no
    /// effect (there is nothing to filter, since raw chunks are sent
    /// as-is), so it is silently forced off with a printed warning.
    #[must_use]
    pub fn new(use_upsample: bool, use_filter: bool, control_hz: f64) -> Self {
        let use_filter = if use_upsample {
            use_filter
        } else {
            if use_filter {
                eprintln!(
                    "Warning: upsample is False, but filter is True. Forcing filter to False."
                );
            }
            false
        };
        Self {
            use_upsample,
            use_filter,
            control_hz,
            canceled_positions: None,
            upsampler: None,
            lowpass: None,
            dynamic_chunk_hz: None,
            target_interval_ns: None,
            target_interval_s: None,
            t_eval: None,
        }
    }

    /// The trajectory tail carried over from a canceled chunk, if any.
    #[must_use]
    pub fn canceled_positions(&self) -> Option<&[Vec<f32>]> {
        self.canceled_positions.as_deref()
    }

    /// Runs one chunk's trajectory: lazily initializes the upsampler and
    /// filter on the first-ever call, applies reset and blend carryover,
    /// then steps through the (possibly upsampled) rows, timing each one
    /// through `env` and reporting it through `on_step` unless a new
    /// chunk or termination is observed first.
    ///
    /// # Errors
    ///
    /// Returns [`RunChunkError`] if blending a canceled trajectory into
    /// this chunk fails, or if upsampling fails (both mirror upstream's
    /// numpy shape-mismatch errors).
    ///
    /// # Panics
    ///
    /// Never panics in practice: the `Option`s populated during lazy
    /// initialization (`upsampler`, `t_eval`, `dynamic_chunk_hz`,
    /// `target_interval_ns`/`_s`) are always set together, before
    /// `use_upsample` can be true anywhere they are read.
    #[allow(clippy::similar_names)] // interval_ns vs. step_interval_ns are genuinely distinct.
    pub fn run_chunk(
        &mut self,
        action_chunk: ActionChunk,
        env: &mut impl Environment,
        mut on_step: impl FnMut(StepOutput),
    ) -> Result<ChunkOutcome, RunChunkError> {
        let ActionChunk {
            mut positions,
            interval_ns,
            cutoff_hz,
            reset,
        } = action_chunk;
        let n_positions = positions.len();

        if self.use_upsample && self.upsampler.is_none() {
            let dynamic_chunk_hz = 1e9 / (interval_ns as f64);
            let horizon_sec = ((n_positions - 1) as f64) / dynamic_chunk_hz;
            self.upsampler = Some(HermiteUpsampler::new(dynamic_chunk_hz, horizon_sec));

            let target_interval_s = 1.0 / self.control_hz;
            let target_interval_ns = (target_interval_s * 1e9) as i64;
            self.t_eval = Some(arange(0.0, horizon_sec + 1e-9, target_interval_s));
            self.dynamic_chunk_hz = Some(dynamic_chunk_hz);
            self.target_interval_ns = Some(target_interval_ns);
            self.target_interval_s = Some(target_interval_s);

            if self.use_filter {
                self.lowpass = Some(BiquadLowpass::new(self.control_hz, cutoff_hz, DEFAULT_Q));
            }
        }

        if reset {
            println!("Resetting trajectory, discarding any previous trajectory.");
            self.canceled_positions = None;
            if let Some(lowpass) = &mut self.lowpass {
                lowpass.reset_state(&positions[0]);
            }
        }

        if let Some(canceled) = self.canceled_positions.take() {
            let (blended, n) = blend(&canceled, &positions)?;
            let mut combined = blended;
            combined.extend_from_slice(&positions[n..]);
            positions = combined;
        }

        let (loop_positions, step_interval_ns, step_interval_s) = if self.use_upsample {
            let upsampler = self.upsampler.as_ref().expect("initialized above");
            let t_eval = self.t_eval.as_ref().expect("initialized above");
            let upsampled = upsampler.upsample(&positions, t_eval)?;
            (
                upsampled,
                self.target_interval_ns.expect("initialized above"),
                self.target_interval_s.expect("initialized above"),
            )
        } else {
            (positions.clone(), interval_ns, (interval_ns as f64) / 1e9)
        };

        let mut base_time = env.now_ns() - step_interval_ns;
        for (i_step, position) in loop_positions.iter().enumerate() {
            let stepped_position = if self.use_filter {
                self.lowpass
                    .as_mut()
                    .map_or_else(|| position.clone(), |lowpass| lowpass.step(position))
            } else {
                position.clone()
            };

            let next_base_time = base_time + step_interval_ns;
            let sleep_time = next_base_time - env.now_ns();
            if sleep_time > 0 {
                env.sleep_ns(sleep_time);
            }
            base_time = next_base_time;
            let timestamp_ns = env.now_ns();

            match env.poll() {
                PendingSignal::Terminate => return Ok(ChunkOutcome::Terminated),
                PendingSignal::NewChunk => {
                    let consumed_raw_steps = if self.use_upsample {
                        let consumed_time_s = (i_step as f64) * step_interval_s;
                        (consumed_time_s * self.dynamic_chunk_hz.expect("initialized above"))
                            as usize
                    } else {
                        i_step
                    };
                    let carry = if consumed_raw_steps < positions.len() {
                        positions[consumed_raw_steps..].to_vec()
                    } else {
                        Vec::new()
                    };
                    self.canceled_positions = Some(carry);
                    return Ok(ChunkOutcome::CancelledForNextChunk);
                }
                PendingSignal::None => {}
            }

            on_step(StepOutput {
                position: stepped_position,
                timestamp_ns,
            });
        }

        Ok(ChunkOutcome::Completed)
    }
}
