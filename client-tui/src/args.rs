use clap::{Parser, Subcommand};

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Connect to a Selecit server")]
    Connect { host: String, port: Option<u16> },
}

#[derive(Parser)]
#[command(about = "Selecit client for connecting to Selecit servers")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}
