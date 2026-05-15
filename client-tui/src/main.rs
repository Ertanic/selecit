use crate::{
    args::{Args, Command},
    proto::query_client::QueryClient,
    tui::App,
};
use clap::Parser;

mod args;
mod proto;
mod tui;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match args.command {
        Command::Connect { host, port } => {
            let server_addr = format!("http://{}:{}", host, port.unwrap_or(1299));
            let client = QueryClient::connect(server_addr.clone()).await.expect("failed to connect");

            let mut terminal = ratatui::init();
            App::new(server_addr, client).await.run(&mut terminal).expect("failed to run app");
            ratatui::restore();
        }
    }
}
