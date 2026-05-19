use clap::{Parser, Subcommand};

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Connect to a Selecit server")]
    Connect {
        /// Server host
        host: String,

        /// Server port
        port: Option<u16>,

        /// Path to the CA certificate
        #[arg(long)]
        ca: Option<String>,

        /// Authentication token
        #[arg(long, short)]
        token: Option<String>,
    },
}

#[derive(Parser)]
#[command(about = "Selecit client for connecting to Selecit servers")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}
