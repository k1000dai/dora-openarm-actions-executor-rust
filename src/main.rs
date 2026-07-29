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

//! Dora adapter for the behavior-compatible actions executor.
//!
//! This binary is only the dora and wall-clock glue: it resolves CLI
//! options, reads and writes real dora events, and measures real time,
//! delegating every decision -- chunk parsing, lazy upsampler/filter
//! initialization, reset and blend handling, per-step timing and
//! cancellation, arm splitting, and output construction -- to the pure
//! library in `src/lib.rs`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dora_node_api::arrow::array::{ArrayRef, ListArray};
use dora_node_api::dora_core::config::DataId;
use dora_node_api::{DoraNode, Event, EventStream, MetadataParameters, Parameter, TryRecvError};
use dora_openarm_actions_executor_rust::{
    ActionChunk, Arms, ChunkOutcome, Environment, PendingSignal, TrajectoryExecutor, build_outputs,
    parse_args, parse_positions,
};

/// Upstream's default low-pass filter cutoff frequency, in Hz, used when
/// an input event carries no `cutoff_hz` metadata.
///
/// A common choice for robotic arm control, balancing smoothness and
/// responsiveness.
const DEFAULT_CUTOFF_HZ: f64 = 15.0;

/// The metadata key carrying the timestamp attached to each sent output.
const TIMESTAMP_KEY: &str = "timestamp";

fn main() -> eyre::Result<()> {
    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    let options = parse_args(&cli_args, std::env::var("ARMS").ok())?;
    let arms = Arms::parse(&options.arms);
    let mut executor =
        TrajectoryExecutor::new(options.upsample, options.filter, options.control_hz);

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let mut pending_event: Option<Event> = None;

    loop {
        let event = match pending_event.take() {
            Some(event) => event,
            None => match events.recv() {
                Some(event) => event,
                None => break,
            },
        };

        // Mirrors upstream's `if event["type"] != "INPUT": break` -- any
        // input, regardless of its id, is treated as an action chunk;
        // anything else ends the node.
        let Event::Input { metadata, data, .. } = event else {
            break;
        };

        let array: ArrayRef = data.into();
        let list = array
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| eyre::eyre!("actions input is not a List<Float32> array"))?;
        let positions = parse_positions(list)?;
        let (interval_ns, cutoff_hz, reset) = extract_metadata(&metadata.parameters)?;

        let action_chunk = ActionChunk {
            positions,
            interval_ns,
            cutoff_hz,
            reset,
        };

        let mut env = RealEnvironment {
            events: &mut events,
            pending: &mut pending_event,
        };
        let mut send_error: Option<eyre::Report> = None;
        let outcome = executor.run_chunk(action_chunk, &mut env, |step| {
            if send_error.is_some() {
                return;
            }
            let (right, left) = arms.split_position(&step.position);
            for (output_id, array) in build_outputs(right.as_deref(), left.as_deref()) {
                let mut out_metadata = MetadataParameters::default();
                out_metadata.insert(
                    TIMESTAMP_KEY.to_owned(),
                    Parameter::Integer(step.timestamp_ns),
                );
                if let Err(error) =
                    node.send_output(DataId::from(output_id.to_owned()), out_metadata, array)
                {
                    send_error = Some(error);
                }
            }
        })?;
        if let Some(error) = send_error {
            return Err(error);
        }

        if outcome == ChunkOutcome::Terminated {
            break;
        }
    }

    Ok(())
}

/// Reads `interval`, `cutoff_hz`, and `reset` out of an input event's
/// metadata parameters.
///
/// Mirrors upstream's `event["metadata"]["interval"]` (required, no
/// default), `event["metadata"].get("cutoff_hz", 15)`, and
/// `event["metadata"].get("reset", False)`.
///
/// # Errors
///
/// Returns an error if `interval` is missing or not an integer, or if
/// `cutoff_hz`/`reset` are present with an unexpected type.
fn extract_metadata(parameters: &MetadataParameters) -> eyre::Result<(i64, f64, bool)> {
    let interval_ns = match parameters.get("interval") {
        Some(Parameter::Integer(value)) => *value,
        Some(other) => eyre::bail!("'interval' metadata has unexpected type: {other:?}"),
        None => eyre::bail!("'interval' metadata is required but missing"),
    };
    let cutoff_hz = match parameters.get("cutoff_hz") {
        Some(Parameter::Integer(value)) => *value as f64,
        Some(Parameter::Float(value)) => *value,
        Some(other) => eyre::bail!("'cutoff_hz' metadata has unexpected type: {other:?}"),
        None => DEFAULT_CUTOFF_HZ,
    };
    let reset = match parameters.get("reset") {
        Some(Parameter::Bool(value)) => *value,
        Some(other) => eyre::bail!("'reset' metadata has unexpected type: {other:?}"),
        None => false,
    };
    Ok((interval_ns, cutoff_hz, reset))
}

/// The real [`Environment`]: wall-clock time via `SystemTime`, real
/// sleeping via `std::thread::sleep`, and a non-blocking peek at the
/// dora event stream. A peeked-but-unconsumed [`Event`] is buffered in
/// `pending` so the next chunk fetch uses it instead of blocking again.
struct RealEnvironment<'a> {
    events: &'a mut EventStream,
    pending: &'a mut Option<Event>,
}

impl Environment for RealEnvironment<'_> {
    fn now_ns(&mut self) -> i64 {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before the UNIX epoch");
        i64::try_from(duration.as_nanos()).expect("current time overflowed i64 nanoseconds")
    }

    fn sleep_ns(&mut self, duration_ns: i64) {
        let nanos = u64::try_from(duration_ns).expect("sleep_ns called with a negative duration");
        std::thread::sleep(Duration::from_nanos(nanos));
    }

    fn poll(&mut self) -> PendingSignal {
        match self.events.try_recv() {
            Ok(event @ Event::Input { .. }) => {
                *self.pending = Some(event);
                PendingSignal::NewChunk
            }
            Ok(_other) => PendingSignal::Terminate,
            Err(TryRecvError::Empty) => PendingSignal::None,
            Err(TryRecvError::Closed) => PendingSignal::Terminate,
        }
    }
}

#[cfg(test)]
mod tests {
    use dora_node_api::{MetadataParameters, Parameter};

    use super::extract_metadata;

    #[test]
    fn interval_is_required() {
        let parameters = MetadataParameters::default();
        assert!(extract_metadata(&parameters).is_err());
    }

    #[test]
    fn cutoff_hz_defaults_to_fifteen() {
        let mut parameters = MetadataParameters::default();
        parameters.insert("interval".to_owned(), Parameter::Integer(1000));
        let (_, cutoff_hz, _) = extract_metadata(&parameters).expect("valid");
        assert!((cutoff_hz - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn reset_defaults_to_false() {
        let mut parameters = MetadataParameters::default();
        parameters.insert("interval".to_owned(), Parameter::Integer(1000));
        let (_, _, reset) = extract_metadata(&parameters).expect("valid");
        assert!(!reset);
    }

    #[test]
    fn all_fields_are_read_when_present() {
        let mut parameters = MetadataParameters::default();
        parameters.insert("interval".to_owned(), Parameter::Integer(2_000_000));
        parameters.insert("cutoff_hz".to_owned(), Parameter::Float(20.0));
        parameters.insert("reset".to_owned(), Parameter::Bool(true));
        let (interval_ns, cutoff_hz, reset) = extract_metadata(&parameters).expect("valid");
        assert_eq!(interval_ns, 2_000_000);
        assert!((cutoff_hz - 20.0).abs() < f64::EPSILON);
        assert!(reset);
    }

    #[test]
    fn integer_cutoff_hz_is_widened_to_float() {
        let mut parameters = MetadataParameters::default();
        parameters.insert("interval".to_owned(), Parameter::Integer(1000));
        parameters.insert("cutoff_hz".to_owned(), Parameter::Integer(30));
        let (_, cutoff_hz, _) = extract_metadata(&parameters).expect("valid");
        assert!((cutoff_hz - 30.0).abs() < f64::EPSILON);
    }
}
