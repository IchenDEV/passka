use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "passka", about = "Local auth broker for AI agents")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage principals (human / agent identities)
    Principal {
        #[command(subcommand)]
        command: PrincipalCommand,
    },
    /// Manage provider accounts bound to the broker
    Account {
        #[command(subcommand)]
        command: AccountCommand,
    },
    /// Request a short-lived access lease
    Request(RequestArgs),
    /// Proxy an HTTP request through the broker using a lease
    Proxy(ProxyArgs),
    /// Complete an OAuth authorization flow for an account
    Auth { account_id: String },
    /// Refresh an OAuth account
    Refresh { account_id: String },
    /// Inspect audit events
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },
    /// Run the local broker daemon
    Broker {
        #[command(subcommand)]
        command: BrokerCommand,
    },
}

#[derive(Subcommand)]
pub enum PrincipalCommand {
    Add {
        name: String,
        #[arg(long)]
        kind: String,
        #[arg(short, long)]
        description: Option<String>,
    },
    List,
}

#[derive(Subcommand)]
pub enum AccountCommand {
    Add {
        name: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        auth: String,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(short, long)]
        description: Option<String>,
        #[arg(long, value_delimiter = ',')]
        scopes: Vec<String>,
    },
    Allow {
        account_id: String,
        #[arg(long)]
        agent: String,
        #[arg(long, value_delimiter = ',')]
        environments: Vec<String>,
        #[arg(long, default_value_t = 300)]
        lease_seconds: i64,
        #[arg(short, long)]
        description: Option<String>,
    },
    List,
    Show { account_id: String },
    Remove { account_id: String },
}

#[derive(Subcommand)]
pub enum AuditCommand {
    List {
        #[arg(long)]
        limit: Option<usize>,
    },
}

#[derive(Subcommand)]
pub enum BrokerCommand {
    Serve {
        #[arg(long, default_value = "127.0.0.1:8478")]
        addr: String,
    },
}

#[derive(Args)]
pub struct RequestArgs {
    #[arg(long)]
    pub principal: String,
    #[arg(long)]
    pub account: String,
    #[arg(long, default_value = "local")]
    pub environment: String,
    #[arg(long, default_value = "broker_request")]
    pub purpose: String,
    #[arg(long, default_value = "cli")]
    pub source: String,
}

#[derive(Args)]
pub struct ProxyArgs {
    #[arg(long)]
    pub lease: String,
    #[arg(long)]
    pub method: String,
    #[arg(long)]
    pub path: String,
    #[arg(long = "header")]
    pub headers: Vec<String>,
    #[arg(long = "extra-lease")]
    pub extra_leases: Vec<String>,
    #[arg(long)]
    pub body: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_allow_is_the_primary_account_auth_flow() {
        let cli = Cli::try_parse_from([
            "passka",
            "account",
            "allow",
            "account-123",
            "--agent",
            "principal:local-agent",
            "--lease-seconds",
            "120",
        ])
        .expect("account allow should parse");

        match cli.command {
            Command::Account {
                command:
                    AccountCommand::Allow {
                        account_id,
                        agent,
                        lease_seconds,
                        ..
                    },
            } => {
                assert_eq!(account_id, "account-123");
                assert_eq!(agent, "principal:local-agent");
                assert_eq!(lease_seconds, 120);
            }
            _ => panic!("unexpected command shape"),
        }
    }

    #[test]
    fn request_uses_account_instead_of_resource_action() {
        let cli = Cli::try_parse_from([
            "passka",
            "request",
            "--principal",
            "principal:local-agent",
            "--account",
            "account-123",
        ])
        .expect("request should parse");

        match cli.command {
            Command::Request(args) => {
                assert_eq!(args.principal, "principal:local-agent");
                assert_eq!(args.account, "account-123");
            }
            _ => panic!("unexpected command shape"),
        }
    }

    #[test]
    fn account_reveal_is_no_longer_available() {
        let result = Cli::try_parse_from([
            "passka",
            "account",
            "reveal",
            "account-123",
            "--field",
            "api_key",
        ]);
        assert!(result.is_err());
    }
}
