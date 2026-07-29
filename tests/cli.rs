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

//! Behavior tests for command-line argument resolution, mirroring
//! upstream's `argparse` configuration.

use dora_openarm_actions_executor_rust::{CliError, CliOptions, parse_args};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn defaults_when_no_args_and_no_env() {
    let options = parse_args(&args(&[]), None).expect("valid");
    assert_eq!(
        options,
        CliOptions {
            arms: "right,left".to_owned(),
            upsample: false,
            filter: false,
            control_hz: 250.0,
        }
    );
}

#[test]
fn env_arms_is_used_when_no_flag_is_given() {
    let options = parse_args(&args(&[]), Some("left".to_owned())).expect("valid");
    assert_eq!(options.arms, "left");
}

#[test]
fn explicit_arms_flag_overrides_the_env_var() {
    let options = parse_args(&args(&["--arms", "right"]), Some("left".to_owned())).expect("valid");
    assert_eq!(options.arms, "right");
}

#[test]
fn arms_accepts_the_equals_form() {
    let options = parse_args(&args(&["--arms=left,right"]), None).expect("valid");
    assert_eq!(options.arms, "left,right");
}

#[test]
fn upsample_flag_sets_upsample_true() {
    let options = parse_args(&args(&["--upsample"]), None).expect("valid");
    assert!(options.upsample);
    assert!(!options.filter);
}

#[test]
fn filter_flag_sets_filter_true() {
    let options = parse_args(&args(&["--filter"]), None).expect("valid");
    assert!(options.filter);
    assert!(!options.upsample);
}

#[test]
fn control_hz_flag_parses_a_float() {
    let options = parse_args(&args(&["--control-hz", "500"]), None).expect("valid");
    assert!((options.control_hz - 500.0).abs() < f64::EPSILON);
}

#[test]
fn control_hz_accepts_the_equals_form() {
    let options = parse_args(&args(&["--control-hz=120.5"]), None).expect("valid");
    assert!((options.control_hz - 120.5).abs() < f64::EPSILON);
}

#[test]
fn repeated_flags_let_the_last_occurrence_win() {
    let options = parse_args(&args(&["--arms", "right", "--arms", "left"]), None).expect("valid");
    assert_eq!(options.arms, "left");
}

#[test]
fn missing_arms_value_is_an_error() {
    let error = parse_args(&args(&["--arms"]), None).unwrap_err();
    assert_eq!(error, CliError::MissingArmsValue);
}

#[test]
fn missing_control_hz_value_is_an_error() {
    let error = parse_args(&args(&["--control-hz"]), None).unwrap_err();
    assert_eq!(error, CliError::MissingControlHzValue);
}

#[test]
fn non_numeric_control_hz_is_an_error() {
    let error = parse_args(&args(&["--control-hz", "abc"]), None).unwrap_err();
    assert_eq!(error, CliError::InvalidControlHz("abc".to_owned()));
}
