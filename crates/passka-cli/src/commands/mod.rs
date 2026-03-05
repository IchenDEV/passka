pub mod add;
pub mod auth;
pub mod exec;
pub mod list;
pub mod manage;
pub mod refresh;

use crate::cli::Command;
use anyhow::Result;

pub fn dispatch(cmd: Command) -> Result<()> {
    match cmd {
        Command::Add {
            name,
            r#type,
            description,
        } => add::run(&name, &r#type, description.as_deref()),
        Command::Auth { name } => auth::run(&name),
        Command::Exec {
            names,
            no_redact,
            command,
        } => exec::run(&names, !no_redact, &command),
        Command::List { r#type } => list::run_list(r#type.as_deref()),
        Command::Show { name } => list::run_show(&name),
        Command::Rm { name } => manage::run_rm(&name),
        Command::Update { name, field } => manage::run_update(&name, &field),
        Command::Refresh { name } => refresh::run(&name),
    }
}
