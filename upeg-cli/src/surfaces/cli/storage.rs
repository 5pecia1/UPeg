use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum StorageAction {
    Status {
        #[arg(long)]
        json: bool,
    },
    Plan {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        ecosystem_prefix: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Apply {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        quiesced: bool,
        #[arg(long)]
        json: bool,
    },
    Verify {
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Rollback {
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        quiesced: bool,
        #[arg(long)]
        json: bool,
    },
}
