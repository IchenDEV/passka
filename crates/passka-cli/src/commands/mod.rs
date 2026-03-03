pub mod add;
pub mod exec;
pub mod get;
pub mod list;
pub mod manage;
pub mod refresh;
pub mod snippet;

use crate::cli::Command;
use anyhow::Result;

pub fn dispatch(cmd: Command) -> Result<()> {
    match cmd {
        Command::Add {
            name,
            r#type,
            description,
        } => add::run(&name, &r#type, description.as_deref()),
        Command::Get { name, field } => get::run(&name, field.as_deref()),
        Command::Exec { name, command } => exec::run(&name, &command),
        Command::List { r#type } => list::run_list(r#type.as_deref()),
        Command::Show { name } => list::run_show(&name),
        Command::Rm { name } => manage::run_rm(&name),
        Command::Update { name, field } => manage::run_update(&name, &field),
        Command::Refresh { name } => refresh::run(&name),
        Command::Snippet { name, lang } => snippet::run_snippet(&name, &lang),
        Command::Env { name } => snippet::run_env(&name),
    }
}
