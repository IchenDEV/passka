mod cli;
mod commands;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    commands::dispatch(args.command)
}
