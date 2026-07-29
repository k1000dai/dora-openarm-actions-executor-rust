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

//! Behavior tests for the per-chunk trajectory scheduler: lazy
//! initialization, reset handling, blend carryover, upsample/raw
//! stepping, filter application, mid-chunk cancellation, and
//! termination.
//!
//! Timing and "has a new event already arrived" are driven through a
//! fake [`Environment`] with a virtual clock and a scripted signal
//! sequence, so these tests need no real sleeping and no dora runtime.

use std::collections::VecDeque;

use dora_openarm_actions_executor_rust::{
    ActionChunk, ChunkOutcome, Environment, PendingSignal, StepOutput, TrajectoryExecutor,
};

struct FakeEnvironment {
    now_ns: i64,
    poll_script: VecDeque<PendingSignal>,
}

impl FakeEnvironment {
    fn new(poll_script: impl IntoIterator<Item = PendingSignal>) -> Self {
        Self {
            now_ns: 0,
            poll_script: poll_script.into_iter().collect(),
        }
    }
}

impl Environment for FakeEnvironment {
    fn now_ns(&mut self) -> i64 {
        self.now_ns
    }

    fn sleep_ns(&mut self, duration_ns: i64) {
        self.now_ns += duration_ns;
    }

    fn poll(&mut self) -> PendingSignal {
        self.poll_script.pop_front().unwrap_or(PendingSignal::None)
    }
}

fn chunk(positions: Vec<Vec<f32>>, interval_ns: i64, reset: bool) -> ActionChunk {
    ActionChunk {
        positions,
        interval_ns,
        cutoff_hz: 15.0,
        reset,
    }
}

fn run_all_none(
    executor: &mut TrajectoryExecutor,
    action_chunk: ActionChunk,
    step_count: usize,
) -> (ChunkOutcome, Vec<StepOutput>) {
    let mut env = FakeEnvironment::new(vec![PendingSignal::None; step_count]);
    let mut steps = Vec::new();
    let outcome = executor
        .run_chunk(action_chunk, &mut env, |step| steps.push(step))
        .expect("run_chunk succeeds");
    (outcome, steps)
}

#[test]
fn without_upsampling_each_raw_row_is_sent_once_at_the_raw_interval() {
    let mut executor = TrajectoryExecutor::new(false, false, 250.0);
    let action_chunk = chunk(vec![vec![1.0], vec![2.0], vec![3.0]], 1000, false);
    let (outcome, steps) = run_all_none(&mut executor, action_chunk, 3);

    assert_eq!(outcome, ChunkOutcome::Completed);
    let positions: Vec<f32> = steps.iter().map(|step| step.position[0]).collect();
    assert_eq!(positions, vec![1.0, 2.0, 3.0]);
    let timestamps: Vec<i64> = steps.iter().map(|step| step.timestamp_ns).collect();
    assert_eq!(timestamps, vec![0, 1000, 2000]);
}

#[test]
fn upsampling_a_two_point_chunk_produces_linear_interpolation() {
    // control_hz=2.0 -> target_interval_s=0.5; interval_ns=1e9 -> dynamic_chunk_hz=1.0.
    // horizon_sec = (2-1)/1.0 = 1.0, so t_eval = [0.0, 0.5, 1.0].
    let mut executor = TrajectoryExecutor::new(true, false, 2.0);
    let action_chunk = chunk(vec![vec![0.0], vec![10.0]], 1_000_000_000, false);
    let (outcome, steps) = run_all_none(&mut executor, action_chunk, 3);

    assert_eq!(outcome, ChunkOutcome::Completed);
    let positions: Vec<f32> = steps.iter().map(|step| step.position[0]).collect();
    assert_eq!(positions, vec![0.0, 5.0, 10.0]);
    let timestamps: Vec<i64> = steps.iter().map(|step| step.timestamp_ns).collect();
    assert_eq!(timestamps, vec![0, 500_000_000, 1_000_000_000]);
}

#[test]
fn cancellation_without_upsampling_carries_over_the_remaining_raw_rows() {
    let mut executor = TrajectoryExecutor::new(false, false, 250.0);
    let action_chunk = chunk(
        vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0]],
        1000,
        false,
    );
    let mut env = FakeEnvironment::new(vec![PendingSignal::None, PendingSignal::NewChunk]);
    let mut steps = Vec::new();
    let outcome = executor
        .run_chunk(action_chunk, &mut env, |step| steps.push(step))
        .expect("run_chunk succeeds");

    assert_eq!(outcome, ChunkOutcome::CancelledForNextChunk);
    // Only the first step is emitted; cancellation is detected right
    // after computing the second step's timestamp, before it is sent.
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].position, vec![1.0]);
    assert_eq!(
        executor.canceled_positions(),
        Some(&[vec![2.0], vec![3.0], vec![4.0]][..])
    );
}

#[test]
fn cancellation_with_upsampling_converts_the_upsample_step_back_to_a_raw_index() {
    // control_hz=2.0 -> target_interval_s=0.5; interval_ns=1e9 -> dynamic_chunk_hz=1.0.
    // horizon_sec = (3-1)/1.0 = 2.0, so t_eval = [0.0, 0.5, 1.0, 1.5, 2.0].
    // Cancelling at i_step=2 (t=1.0): consumed_time_s = 2*0.5 = 1.0,
    // consumed_raw_steps = int(1.0 * 1.0) = 1, so raw rows [1..] carry
    // over, not upsampled rows [2..].
    let mut executor = TrajectoryExecutor::new(true, false, 2.0);
    let action_chunk = chunk(
        vec![vec![0.0], vec![10.0], vec![20.0]],
        1_000_000_000,
        false,
    );
    let mut env = FakeEnvironment::new(vec![
        PendingSignal::None,
        PendingSignal::None,
        PendingSignal::NewChunk,
    ]);
    let mut steps = Vec::new();
    let outcome = executor
        .run_chunk(action_chunk, &mut env, |step| steps.push(step))
        .expect("run_chunk succeeds");

    assert_eq!(outcome, ChunkOutcome::CancelledForNextChunk);
    assert_eq!(steps.len(), 2);
    assert_eq!(
        executor.canceled_positions(),
        Some(&[vec![10.0], vec![20.0]][..])
    );
}

#[test]
fn terminate_signal_stops_immediately_without_sending_that_step() {
    let mut executor = TrajectoryExecutor::new(false, false, 250.0);
    let action_chunk = chunk(vec![vec![1.0], vec![2.0], vec![3.0]], 1000, false);
    let mut env = FakeEnvironment::new(vec![PendingSignal::None, PendingSignal::Terminate]);
    let mut steps = Vec::new();
    let outcome = executor
        .run_chunk(action_chunk, &mut env, |step| steps.push(step))
        .expect("run_chunk succeeds");

    assert_eq!(outcome, ChunkOutcome::Terminated);
    assert_eq!(steps.len(), 1);
}

#[test]
fn reset_discards_any_previously_canceled_trajectory() {
    let mut executor = TrajectoryExecutor::new(false, false, 250.0);

    // First chunk is cancelled after its first row, leaving a carryover.
    let first_chunk = chunk(vec![vec![1.0], vec![2.0], vec![3.0]], 1000, false);
    let mut env = FakeEnvironment::new(vec![PendingSignal::None, PendingSignal::NewChunk]);
    executor
        .run_chunk(first_chunk, &mut env, |_| {})
        .expect("run_chunk succeeds");
    assert!(executor.canceled_positions().is_some());

    // A reset chunk must discard that carryover instead of blending it in.
    let reset_chunk = chunk(vec![vec![100.0], vec![200.0]], 1000, true);
    let (outcome, steps) = run_all_none(&mut executor, reset_chunk, 2);

    assert_eq!(outcome, ChunkOutcome::Completed);
    let positions: Vec<f32> = steps.iter().map(|step| step.position[0]).collect();
    assert_eq!(positions, vec![100.0, 200.0]);
}

#[test]
fn a_non_reset_chunk_blends_in_the_canceled_trajectory() {
    let mut executor = TrajectoryExecutor::new(false, false, 250.0);

    let first_chunk = chunk(vec![vec![1.0], vec![2.0], vec![3.0]], 1000, false);
    let mut env = FakeEnvironment::new(vec![PendingSignal::None, PendingSignal::NewChunk]);
    executor
        .run_chunk(first_chunk, &mut env, |_| {})
        .expect("run_chunk succeeds");
    assert_eq!(
        executor.canceled_positions(),
        Some(&[vec![2.0], vec![3.0]][..])
    );

    // linspace(1, 0, 2) = [1.0, 0.0]: row 0 stays canceled[0]=2.0, row 1
    // becomes next[1]=200.0.
    let next_chunk = chunk(vec![vec![100.0], vec![200.0], vec![300.0]], 1000, false);
    let (outcome, steps) = run_all_none(&mut executor, next_chunk, 3);

    assert_eq!(outcome, ChunkOutcome::Completed);
    let positions: Vec<f32> = steps.iter().map(|step| step.position[0]).collect();
    assert_eq!(positions, vec![2.0, 200.0, 300.0]);
}

#[test]
fn filter_is_forced_off_when_upsampling_is_disabled() {
    // Mirrors upstream's warning-and-override: `filter=true` has no
    // effect unless `upsample` is also enabled, so raw rows pass through
    // completely unfiltered.
    let mut executor = TrajectoryExecutor::new(false, true, 250.0);
    let action_chunk = chunk(vec![vec![0.0], vec![100.0]], 1000, false);
    let (_, steps) = run_all_none(&mut executor, action_chunk, 2);
    let positions: Vec<f32> = steps.iter().map(|step| step.position[0]).collect();
    assert_eq!(positions, vec![0.0, 100.0]);
}

#[test]
fn reset_resnaps_the_filter_to_the_reset_chunks_first_raw_position() {
    // The filter's state is reset to `positions[0]` *before* upsampling.
    // Hermite interpolation passes exactly through a chunk's own samples
    // at their own timestamps, so the first upsampled row equals
    // `positions[0]` exactly, and stepping a freshly reset filter with
    // that same value returns it unchanged (its DC gain is exactly 1).
    let mut executor = TrajectoryExecutor::new(true, true, 1.0);

    let first_chunk = chunk(vec![vec![0.0], vec![10.0]], 1_000_000_000, false);
    let (_, _) = run_all_none(&mut executor, first_chunk, 2);

    let reset_chunk = chunk(vec![vec![7.0], vec![20.0]], 1_000_000_000, true);
    let (outcome, steps) = run_all_none(&mut executor, reset_chunk, 2);

    assert_eq!(outcome, ChunkOutcome::Completed);
    assert!((steps[0].position[0] - 7.0).abs() < 1e-6);
}

#[test]
fn cutoff_hz_is_applied_only_on_the_very_first_chunk() {
    // The low-pass filter is constructed once, lazily, on the first
    // chunk ever processed; later chunks' `cutoff_hz` is read but has no
    // effect. An executor whose second chunk claims a very different
    // cutoff must still filter identically to one that never changes
    // cutoff at all.
    let mut varying = TrajectoryExecutor::new(true, true, 4.0);
    let mut fixed = TrajectoryExecutor::new(true, true, 4.0);

    let first = || chunk(vec![vec![0.0], vec![10.0], vec![0.0]], 250_000_000, false);
    run_all_none(&mut varying, first(), 3);
    run_all_none(&mut fixed, first(), 3);

    let mut second_varying = chunk(vec![vec![0.0], vec![10.0], vec![0.0]], 250_000_000, false);
    second_varying.cutoff_hz = 1.0;
    let second_fixed = chunk(vec![vec![0.0], vec![10.0], vec![0.0]], 250_000_000, false);

    let (_, varying_steps) = run_all_none(&mut varying, second_varying, 3);
    let (_, fixed_steps) = run_all_none(&mut fixed, second_fixed, 3);

    for (a, b) in varying_steps.iter().zip(fixed_steps.iter()) {
        assert!((a.position[0] - b.position[0]).abs() < 1e-9);
    }
}
