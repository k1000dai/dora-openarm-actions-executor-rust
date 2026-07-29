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

//! Command-line argument resolution, mirroring upstream's `argparse`
//! configuration.
//!
//! Parsing takes its arguments and the `ARMS` environment variable's
//! value as plain parameters (rather than reading `std::env` itself), so
//! it is fully testable without mutating global process state.

use std::fmt;

/// The default arms configuration, matching upstream's
/// `os.getenv("ARMS", "right,left")` fallback.
const DEFAULT_ARMS: &str = "right,left";

/// The default motor control frequency, in Hz, matching upstream's
/// `--control-hz` default.
const DEFAULT_CONTROL_HZ: f64 = 250.0;

/// The resolved command-line configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct CliOptions {
    /// The arms configuration string, e.g. `"right,left"`.
    pub arms: String,
    /// Whether to upsample the actions.
    pub upsample: bool,
    /// Whether to apply the low-pass filter (only meaningful alongside
    /// `upsample`).
    pub filter: bool,
    /// The motor control frequency, in Hz.
    pub control_hz: f64,
}

/// An error resolving CLI options.
#[derive(Debug, Clone, PartialEq)]
pub enum CliError {
    /// `--arms` was the last argument, with no value following it.
    MissingArmsValue,
    /// `--control-hz` was the last argument, with no value following it.
    MissingControlHzValue,
    /// `--control-hz`'s value did not parse as a float.
    InvalidControlHz(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingArmsValue => write!(f, "--arms requires a value"),
            Self::MissingControlHzValue => write!(f, "--control-hz requires a value"),
            Self::InvalidControlHz(value) => {
                write!(f, "--control-hz value {value:?} is not a valid number")
            }
        }
    }
}

impl std::error::Error for CliError {}

/// Resolves CLI options from `args` (the program's own arguments, not
/// `argv[0]`) and the `ARMS` environment variable's value, if set.
///
/// Accepts both `--flag value` and `--flag=value` forms for `--arms` and
/// `--control-hz`. The last occurrence of a flag wins, matching
/// `argparse`'s left-to-right overwrite semantics. An explicit `--arms`
/// takes priority over `env_arms`, matching upstream's `argparse`
/// default of `os.getenv("ARMS", "right,left")`.
///
/// # Errors
///
/// Returns [`CliError::MissingArmsValue`] or
/// [`CliError::MissingControlHzValue`] if the respective flag is the
/// last argument, and [`CliError::InvalidControlHz`] if
/// `--control-hz`'s value does not parse as a float.
pub fn parse_args(args: &[String], env_arms: Option<String>) -> Result<CliOptions, CliError> {
    let mut arms = env_arms.unwrap_or_else(|| DEFAULT_ARMS.to_owned());
    let mut upsample = false;
    let mut filter = false;
    let mut control_hz = DEFAULT_CONTROL_HZ;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--arms=") {
            value.clone_into(&mut arms);
        } else if arg == "--arms" {
            arms = iter.next().cloned().ok_or(CliError::MissingArmsValue)?;
        } else if arg == "--upsample" {
            upsample = true;
        } else if arg == "--filter" {
            filter = true;
        } else if let Some(value) = arg.strip_prefix("--control-hz=") {
            control_hz = value
                .parse()
                .map_err(|_| CliError::InvalidControlHz(value.to_owned()))?;
        } else if arg == "--control-hz" {
            let value = iter.next().ok_or(CliError::MissingControlHzValue)?;
            control_hz = value
                .parse()
                .map_err(|_| CliError::InvalidControlHz(value.clone()))?;
        }
    }

    Ok(CliOptions {
        arms,
        upsample,
        filter,
        control_hz,
    })
}
