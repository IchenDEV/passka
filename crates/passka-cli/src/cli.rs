use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "passka", about = "AI-agent friendly credential manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add a new credential (interactive guided flow)
    Add {
        name: String,
        #[arg(short, long)]
        r#type: String,
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Authorize an OAuth credential (browser-based flow)
    Auth { name: String },
    /// Inject credentials as env vars and execute a command
    Exec {
        /// One or more credential names to inject
        names: Vec<String>,
        /// Redact sensitive values from child process output
        #[arg(long)]
        redact: bool,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// List stored credentials (metadata only)
    List {
        #[arg(short, long)]
        r#type: Option<String>,
    },
    /// Show credential metadata with masked values
    Show { name: String },
    /// Remove a credential
    Rm { name: String },
    /// Update a credential field
    Update {
        name: String,
        #[arg(short, long)]
        field: String,
    },
    /// Refresh an OAuth token
    Refresh { name: String },
}
