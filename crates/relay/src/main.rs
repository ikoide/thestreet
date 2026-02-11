use clap::Parser;

use street_common::config::{load_config, RelayConfig};
use street_wallet::mock::MockWallet;

use crate::server::RelayServer;
use crate::state::ServerState;
use crate::storage::Storage;

mod server;
mod state;
mod storage;

#[derive(Parser, Debug)]
#[command(name = "street-relay")]
struct Args {
    #[arg(long, default_value = "config/relay.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config: RelayConfig = load_config(&args.config)?;
    let storage = Storage::new(&config.data_dir)?;
    let users = storage.load_users()?;
    let rooms = storage.load_rooms()?;
    let state = ServerState::new(users, rooms);
    let wallet = MockWallet::new();

    let server = RelayServer::new(config, storage, state, wallet);
    server.run().await
}
