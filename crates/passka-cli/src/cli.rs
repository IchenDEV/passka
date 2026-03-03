use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "passka", about = "AI-agent friendly credential manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add a new credential (interactive secure input)
    Add {
        name: String,
        #[arg(short, long)]
        r#type: String,
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Get a credential value (outputs to stdout)
    Get {
        name: String,
        #[arg(short, long)]
        field: Option<String>,
    },
    /// Inject credential as env vars and execute a command
    Exec {
        name: String,
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
    /// Generate a code snippet for using a credential
    Snippet {
        name: String,
        #[arg(short, long, default_value = "bash")]
        lang: String,
    },
    /// Output export statements for a credential
    Env { name: String },
}
