//! The `External` invoker: everything about turning a manifest's
//! `command` + `args_template` into a spawned child process and its
//! canonical [`upeg_core::ToolResult`].
//!
//! Layering inside this module:
//!
//! * [`invoker`] builds the dispatcher closure — arg templating,
//!   working directory, environment, timeout.
//! * [`capture`] / [`child`] / [`drain`] / [`cancel`] / [`scope`] run
//!   the child under bounded, deadlock-free, group-terminating capture.
//! * [`color`] declares what the child is told about color support,
//!   [`pty`] gives it a real terminal when the convention is not enough,
//!   and [`progress`] taps the read loop so a surface can show output
//!   while the child is still running.
//! * [`outcome`] turns a non-zero exit, a signal, or a timeout into the
//!   structured failure envelope every surface renders.

mod cancel;
mod capture;
mod child;
pub(crate) mod color;
mod drain;
mod error;
mod invoker;
mod outcome;
mod progress;
pub(crate) mod pty;
mod scope;
pub(crate) mod template;

use std::process::Command;
use std::time::Duration;

use upeg_core::MAX_UNTRUSTED_OUTPUT_WIRE_BYTES;

pub(crate) use invoker::external_dispatcher_for;

use capture::{RunBudget, RunControls};
use error::{CaptureCompletion, CapturedOutput, ExternalProcessError};

#[cfg(test)]
use error::{CaptureLimit, OutputStream};

fn run_command(
    command: &mut Command,
    timeout: Option<Duration>,
    controls: RunControls,
) -> Result<CapturedOutput, ExternalProcessError> {
    capture::run(
        command,
        error::CaptureLimit::new(MAX_UNTRUSTED_OUTPUT_WIRE_BYTES),
        RunBudget::from_timeout(timeout),
        controls,
    )
}

#[cfg(test)]
fn run_command_with_limit(
    command: &mut Command,
    limit: CaptureLimit,
) -> Result<CapturedOutput, ExternalProcessError> {
    capture::run(
        command,
        limit,
        RunBudget::unbounded(),
        RunControls::default(),
    )
}

#[cfg(test)]
mod cancellation_tests;
#[cfg(test)]
mod color_tests;
#[cfg(test)]
mod external_tests;
#[cfg(test)]
mod liveness_tests;
#[cfg(all(test, target_os = "linux"))]
mod process_test_support;
#[cfg(all(test, unix))]
mod pty_tests;
#[cfg(test)]
mod streaming_tests;
