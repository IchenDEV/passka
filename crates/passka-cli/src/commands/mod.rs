pub mod access;
pub mod account;
pub mod audit;
pub mod auth;
pub mod broker;
pub mod principal;
pub mod refresh;

use crate::cli::{AccountCommand, AuditCommand, BrokerCommand, Command, PrincipalCommand};
use anyhow::Result;

pub fn dispatch(cmd: Command) -> Result<()> {
    match cmd {
        Command::Principal { command } => match command {
            PrincipalCommand::Add {
                name,
                kind,
                description,
            } => principal::add(&name, &kind, description.as_deref()),
            PrincipalCommand::List => principal::list(),
        },
        Command::Account { command } => match command {
            AccountCommand::Add {
                name,
                provider,
                auth,
                base_url,
                description,
                scopes,
            } => account::add(
                &name,
                &provider,
                &auth,
                base_url.as_deref(),
                description.as_deref(),
                &scopes,
            ),
            AccountCommand::Allow {
                account_id,
                agent,
                environments,
                lease_seconds,
                description,
            } => account::allow(
                &account_id,
                &agent,
                &environments,
                lease_seconds,
                description.as_deref(),
            ),
            AccountCommand::List => account::list(),
            AccountCommand::Show { account_id } => account::show(&account_id),
            AccountCommand::Remove { account_id } => account::remove(&account_id),
        },
        Command::Request(args) => access::request(args),
        Command::Proxy(args) => access::proxy(args),
        Command::Auth { account_id } => auth::run(&account_id),
        Command::Refresh { account_id } => refresh::run(&account_id),
        Command::Audit { command } => match command {
            AuditCommand::List { limit } => audit::list(limit),
        },
        Command::Broker { command } => match command {
            BrokerCommand::Serve { addr } => broker::serve(&addr),
        },
    }
}
