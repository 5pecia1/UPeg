use clap::{Args, Parser as _, Subcommand, ValueEnum, error::ErrorKind};

use super::{BoardAction, BoardScopedCli, Cli, Command};

impl Cli {
    /// The external board route receives help flags as raw arguments.
    /// Render them without opening storage or loading runtime tools.
    pub fn board_scoped_help(&self) -> Option<String> {
        let Some(Command::Board {
            action: BoardAction::Scoped(argv) | BoardAction::On { argv },
        }) = &self.command
        else {
            return None;
        };
        let (board, args) = argv.split_first()?;
        let _board = upeg_core::BoardKey::parse(board).ok()?;
        if !args
            .iter()
            .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "help"))
        {
            return None;
        }
        let error = BoardScopedCli::try_parse_from(args).err()?;
        (error.kind() == ErrorKind::DisplayHelp).then(|| error.to_string())
    }
}

#[derive(Args, Debug)]
pub struct PinEditArgs {
    /// Exact instance ID from pin-list.
    pub pin_id: String,
    /// Override the tile size as COLSxROWS.
    #[arg(long, value_name = "COLSxROWS", conflicts_with = "reset_size")]
    pub size: Option<String>,
    /// Restore the tool manifest's default size.
    #[arg(long)]
    pub reset_size: bool,
    /// Override the tile color as #RRGGBB.
    #[arg(long, value_name = "HEX", conflicts_with = "reset_color")]
    pub color: Option<String>,
    /// Restore the default tile color.
    #[arg(long)]
    pub reset_color: bool,
    /// Emit the resulting pin as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SettingKey {
    Locale,
    Theme,
    Accent,
}

#[derive(Subcommand, Debug)]
pub enum SettingsAction {
    /// Read one setting, or read all three by omitting KEY.
    Get {
        key: Option<SettingKey>,
        #[arg(long)]
        json: bool,
    },
    /// Set locale (en/ko), theme (light/dark), or accent (green/amber/cyan/pink).
    Set {
        key: SettingKey,
        value: String,
        #[arg(long)]
        json: bool,
    },
}
