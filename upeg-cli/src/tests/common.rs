use crate::Cli;
use clap::Parser;

pub(crate) fn parse(argv: &[&str]) -> Cli {
    Cli::parse_from(argv)
}

/// Keep process-global home overrides from changing while a CLI command runs.
pub(crate) fn run(cli: Cli) -> Result<String, crate::CliError> {
    let _guard = crate::test_support::pegboard_home_test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    crate::run(cli)
}
