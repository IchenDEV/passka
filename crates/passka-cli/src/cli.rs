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
    /// Create and inspect broker policies
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
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
    List,
    Show { account_id: String },
    Reveal {
        account_id: String,
        #[arg(long)]
        field: String,
        #[arg(long, default_value = "principal:local-human")]
        principal: String,
        #[arg(long)]
        raw: bool,
    },
    Remove { account_id: String },
}

#[derive(Subcommand)]
pub enum PolicyCommand {
    Allow {
        #[arg(long)]
        principal: String,
        #[arg(long)]
        account: String,
        #[arg(long)]
        resource: String,
        #[arg(long, value_delimiter = ',')]
        actions: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        environments: Vec<String>,
        #[arg(long, default_value_t = 300)]
        lease_seconds: i64,
        #[arg(long)]
        allow_secret_reveal: bool,
        #[arg(short, long)]
        description: Option<String>,
    },
    List,
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
    pub resource: String,
    #[arg(long)]
    pub action: String,
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
    #[arg(long)]
    pub body: Option<String>,
}
