pub mod access;
pub mod account;
pub mod audit;
pub mod auth;
pub mod broker;
pub mod policy;
pub mod principal;
pub mod refresh;

use crate::cli::{
    AccountCommand, AuditCommand, BrokerCommand, Command, PolicyCommand, PrincipalCommand,
};
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
            AccountCommand::List => account::list(),
            AccountCommand::Show { account_id } => account::show(&account_id),
            AccountCommand::Reveal {
                account_id,
                field,
                principal,
                raw,
            } => account::reveal(&account_id, &field, &principal, raw),
            AccountCommand::Remove { account_id } => account::remove(&account_id),
        },
        Command::Policy { command } => match command {
            PolicyCommand::Allow {
                principal,
                account,
                resource,
                actions,
                environments,
                lease_seconds,
                allow_secret_reveal,
                description,
            } => policy::allow(
                &principal,
                &account,
                &resource,
                &actions,
                &environments,
                lease_seconds,
                allow_secret_reveal,
                description.as_deref(),
            ),
            PolicyCommand::List => policy::list(),
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
