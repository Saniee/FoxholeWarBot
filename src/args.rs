use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Name of the person to greet
    #[arg(long, action=ArgAction::SetTrue)]
    pub clear_commands: bool,

    #[arg(short, long, action=ArgAction::SetTrue)]
    pub local: bool,
}