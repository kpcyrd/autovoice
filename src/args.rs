use crate::errors::*;
use clap::{ArgAction, CommandFactory, Parser};
use clap_complete::Shell;
use std::io::stdout;

#[derive(Debug, Parser)]
#[command(version)]
pub struct Args {
    /// Increase logging output (can be used multiple times)
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub verbose: u8,

    /// irc server
    #[arg(long)]
    pub server: String,
    /// irc user nickname
    #[arg(long)]
    pub nickname: String,
    /// irc user password
    #[arg(long, env = "IRC_USER_PASSWORD")]
    pub password: Option<String>,
    /// irc channel to moderate
    #[arg(long)]
    pub channel: String,
    /// Wait this many seconds before promoting a user
    #[arg(long, group = "promote_after")]
    pub promote_after_seconds: Option<u64>,
    /// Wait this many minutes before promoting a user
    #[arg(long, group = "promote_after")]
    pub promote_after_minutes: Option<u64>,
    /// Wait this many hours before promoting a user
    #[arg(long, group = "promote_after")]
    pub promote_after_hours: Option<u64>,
}
/// Generate shell completions
#[derive(Debug, Parser)]
pub struct Completions {
    pub shell: Shell,
}

pub fn gen_completions(args: &Completions) -> Result<()> {
    clap_complete::generate(args.shell, &mut Args::command(), "autovoice", &mut stdout());
    Ok(())
}
