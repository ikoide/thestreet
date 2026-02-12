use std::fs;
use std::path::Path;

use clap::Parser;
use tokio::sync::mpsc;

use street_common::config::{load_config, ClientConfig};
use street_common::crypto::{decode_signing_key, Keypair};

use crate::boot::boot_and_connect;
use crate::crypto::identity_from_signing_key;
use crate::ui::{run_ui, AppState};

mod input;
mod net;
mod render;
mod ui;
mod boot;
mod crypto;

#[derive(Parser, Debug)]
#[command(name = "street-client")]
struct Args {
    #[arg(long, default_value = "config/client.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config: ClientConfig = load_config(&args.config)?;
    let signing_key = load_or_create_key(&config.identity_key_path)?;

    let x_identity = identity_from_signing_key(&signing_key);
    let connection = boot_and_connect(&config, &signing_key, &x_identity.public_b64).await?;
    let app_state = AppState::from_welcome(
        &connection.welcome,
        x_identity.secret.to_bytes(),
        x_identity.public_b64,
    );

    let (input_tx, input_rx) = mpsc::unbounded_channel();
    input::spawn_input_reader(input_tx);

    run_ui(
        app_state,
        connection.incoming,
        connection.outgoing,
        signing_key,
        input_rx,
    )
    .await
}

fn load_or_create_key(path: &str) -> anyhow::Result<ed25519_dalek::SigningKey> {
    let key_path = Path::new(path);
    if key_path.exists() {
        let content = fs::read_to_string(key_path)?;
        let signing = decode_signing_key(content.trim())?;
        return Ok(signing);
    }
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let keypair = Keypair::generate();
    fs::write(key_path, keypair.signing_key_base64())?;
    Ok(keypair.signing)
}
