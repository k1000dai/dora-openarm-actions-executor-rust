# dora-openarm-actions-executor-rust

A [dora-rs](https://dora-rs.ai/) node that executes timestamped actions for
OpenArm.

This is a behavior-compatible Rust port of
[dora-openarm-actions-executor](https://github.com/enactic/dora-openarm-actions-executor),
the original Python implementation.

## Behavior

On every `INPUT` event -- regardless of its input id -- the node treats the
event's value as one action chunk (a variable-size `List<Float32>`, one row
per action step) and its metadata as `interval` (required, nanoseconds
between raw rows), `cutoff_hz` (optional, default `15.0`), and `reset`
(optional, default `false`):

1. **Lazily initializes on the first chunk ever processed.** If
   `--upsample` is set, the first chunk's `interval` and row count fix the
   cubic Hermite upsampler's chunk rate and horizon for the rest of the
   run; later chunks are assumed to share that shape. If `--filter` is
   also set, the low-pass filter is constructed from that same first
   chunk's `cutoff_hz` -- **later chunks' `cutoff_hz` has no effect**,
   matching upstream exactly.
2. **Handles episode resets.** A `reset` chunk discards any trajectory
   canceled by a previous chunk (see below) instead of blending it in, and
   re-anchors the low-pass filter's state to the reset chunk's own first
   raw row, so the filter doesn't pull the new episode's first samples
   toward the previous episode's final pose.
3. **Blends a canceled trajectory into the next chunk.** If a chunk was
   interrupted mid-playback by an already-arrived chunk (see below), the
   next non-reset chunk crossfades the canceled tail into its own leading
   rows (linear ramp from fully canceled to fully new) before playing.
4. **Optionally upsamples and filters.** With `--upsample`, each chunk is
   resampled from its raw rate up to `--control-hz` using cubic Hermite
   spline interpolation (monotonicity-preserving: a local extremum's
   derivative is zeroed rather than overshooting). With `--filter` (only
   meaningful alongside `--upsample`; forced off otherwise, with a
   warning), the upsampled stream is smoothed with a Tustin biquad
   low-pass filter.
5. **Steps through the (possibly upsampled) rows in real time**, sending
   one output per configured arm per step, timed to the raw or target
   interval. If a new chunk has already arrived by the time a step is due,
   the current trajectory is canceled -- its remaining raw rows are saved
   for the next chunk's blend step -- and the node moves on to that new
   chunk immediately. If the node should terminate (the dora event stream
   closes, or delivers anything other than an input), it stops
   immediately, discarding whatever remains of the current trajectory.
6. **Splits and sends per-arm outputs dynamically.** Each configured arm
   (`--arms`, e.g. `"right,left"`, `"right"`, or `"left"`; matched by exact
   token, like upstream's plain Python list membership check) takes 8
   elements (7 joints + 1 gripper) off the step's joined position vector,
   in right-then-left order, and is sent on its own output id
   (`move_position_right`/`move_position_left`) only if enabled. If both
   arms are enabled, each output is a `StructArray` of `new_position` (its
   own position) and `other_arm_position` (the other arm's position at
   that same step); if only one arm is enabled, its output is a plain
   `Float32` array.

## Architecture

The port separates the pure chunk-scheduling logic from the dora adapter,
so timing, cancellation, blending, reset, and filter-state behavior are
all testable without a running dataflow:

- `src/cli.rs` -- resolves `--arms`/`--upsample`/`--filter`/`--control-hz`
  from arguments and the `ARMS` environment variable.
- `src/arms.rs` -- `Arms`, arm-membership parsing and per-step position
  splitting.
- `src/chunk.rs` -- `ActionChunk`, parsing of the `actions` input's
  `List<Float32>` value array into position rows.
- `src/blend.rs` -- crossfade blending of a canceled trajectory into the
  next chunk.
- `src/upsample.rs` -- `HermiteUpsampler`, cubic Hermite spline upsampling.
- `src/lowpass.rs` -- `BiquadLowpass`, the Tustin biquad low-pass filter.
- `src/numeric.rs` -- a `numpy.arange`-equivalent helper.
- `src/executor.rs` -- `TrajectoryExecutor`, the stateful per-chunk
  scheduler (lazy init, reset, blend, upsample/raw stepping, filtering,
  cancellation, termination), driven through the small `Environment`
  trait (`now_ns`/`sleep_ns`/`poll`) instead of touching a clock or dora
  directly -- so its tests supply a fake, virtual-time environment.
- `src/output.rs` -- construction of the `move_position_right`/
  `move_position_left` output arrays.
- `src/main.rs` -- the dora adapter. It has no dora dependency in the
  library above; only `main.rs` touches `dora-node-api`, wall-clock time,
  and thread sleeping, implementing `Environment` against the real clock
  and a non-blocking peek (`EventStream::try_recv`) at the dora event
  stream.

## Usage

Build the executable:

```console
$ cargo build --release
```

Reference it from a dataflow YAML file:

```yaml
nodes:
  - id: actions-executor
    path: dora-openarm-actions-executor
    args: --upsample --filter --control-hz 250
    env:
      ARMS: right,left
    inputs:
      actions: policy-server-client/actions
    outputs:
      - move_position_right
      - move_position_left
```

Then run the dataflow:

```console
$ dora run dataflow.yaml
```

## Migration mapping

| Python (upstream)                                                     | Rust (this port)                                                                          |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `src/dora_openarm_actions_executor/main.py`                            | `src/main.rs` (dora adapter) + `src/{cli,arms,chunk,blend,upsample,lowpass,numeric,executor,output}.rs` |
| `argparse` `--arms` / `os.getenv("ARMS", ...)`                         | `cli::parse_args`                                                                          |
| `argparse` `--upsample`/`--filter`/`--control-hz`                     | `cli::parse_args` (`CliOptions`)                                                           |
| `"right" in arms` / `"left" in arms`, `n_elements = 8` offset walk     | `arms::Arms::parse`, `arms::Arms::split_position`                                          |
| `event["value"].values.to_numpy().reshape(n_positions, pos_shape)`     | `chunk::parse_positions`                                                                   |
| `event["metadata"]["interval"]` / `.get("cutoff_hz", 15)` / `.get("reset", False)` | `main::extract_metadata`                                                       |
| `blend(canceled_positions, next_positions)`                            | `blend::blend`                                                                             |
| `HermiteUpsampler`                                                      | `upsample::HermiteUpsampler`, `upsample::compute_slopes`                                   |
| `BiquadLowpass`                                                         | `lowpass::BiquadLowpass`                                                                   |
| `np.arange(...)`                                                        | `numeric::arange`                                                                          |
| `_main_executor`'s per-event state and step loop                       | `executor::TrajectoryExecutor::run_chunk`, driven by `executor::Environment`               |
| `if not events.empty(): ... break` (mid-chunk cancellation)            | `Environment::poll` returning `PendingSignal::NewChunk`                                    |
| `_main_dora`'s `if event["type"] != "INPUT": break` + `executor_task.cancel()` | `Environment::poll`/the main loop treating any non-`Input` event as `PendingSignal::Terminate` |
| `node.send_output("move_position_right"/"_left", ..., {"timestamp": ...})` | `output::build_outputs` + `main.rs`'s `node.send_output`                                |
| `pytest` (upstream has none)                                            | `cargo test --all-targets`                                                                 |
| `ruff` (pydocstyle, pyupgrade)                                          | `cargo fmt` + `cargo clippy` (`pedantic`, `missing_docs`)                                   |

## Compatibility notes

- **Arrow version.** `arrow` is pinned to the version that `dora-node-api`
  0.5.0 depends on (`54.2.1`), so `ArrayRef` values cross the
  library/adapter boundary as the same type.
- **Telemetry features disabled.** `dora-node-api` is used with
  `default-features = false`; this node needs no telemetry, so the
  optional features stay off.
- **No upstream tests.** The original Python implementation ships no test
  suite; this port's behavior tests were derived directly from reading
  `main.py`, plus independently hand-derived (or, for the biquad filter,
  independently computed from the same RBJ cookbook formula upstream
  implements) numeric reference values -- not from any upstream fixture.
- **`cutoff_hz` is applied once.** The low-pass filter is constructed
  lazily from the *first* chunk's `cutoff_hz`; every later chunk's
  `cutoff_hz` metadata is read but has no effect, exactly matching
  upstream's own one-time `BiquadLowpass(...)` construction.
- **Arms membership is exact-token, not substring or trimmed.** `--arms`
  is split on `,` and each token is compared for exact equality to
  `"right"`/`"left"`, matching Python's `"right" in arms` list-membership
  check -- `"right "` (with trailing space) does not enable the right arm.
- **CLI parsing is hand-rolled, not `clap`.** `cli::parse_args` takes its
  arguments and the `ARMS` environment variable's value as plain
  parameters rather than reading `std::env` itself, so its `argparse`-
  compatible defaulting and override behavior is fully testable without
  mutating global process state.
- **Termination is checked at discrete points, matching upstream's own
  granularity.** Upstream's `asyncio` cancellation can only actually
  interrupt the executor task at an `await` point (the per-step sleep, or
  the next chunk fetch) -- if a step's sleep is skipped because it is
  already behind schedule, there is no `await` for cancellation to land
  on until the next one. This port's `Environment::poll` is checked at
  the same points (once per step, after that step's sleep) for the same
  reason: exact preemption timing is a property of Python's own scheduler
  racing real time, not part of the business logic this port preserves.
- **Wall-clock timestamps.** Upstream's `time.time_ns()` (wall-clock,
  not monotonic) is matched by `SystemTime::now()` since downstream
  consumers of the `timestamp` metadata expect a real epoch timestamp.

## Development

```console
$ cargo fmt --check
$ cargo clippy --all-targets --all-features -- -D warnings
$ cargo test --all-targets
$ cargo build --release
```

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details,
and [NOTICE](NOTICE) for attribution.

Copyright 2026 Enactic, Inc.

## Code of Conduct

All participation in the OpenArm project is governed by the upstream
[Code of Conduct](https://github.com/enactic/dora-openarm-actions-executor/blob/main/CODE_OF_CONDUCT.md).
