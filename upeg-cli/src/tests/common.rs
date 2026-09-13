use crate::Cli;
use clap::Parser;

pub(crate) fn parse(argv: &[&str]) -> Cli {
    Cli::parse_from(argv)
}
