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
    #[arg(long)]
    config: Option<String>,
}

const DEFAULT_CONFIG_PATH: &str = "config/client.toml";
const DEFAULT_IDENTITY_KEY_PATH: &str = "config/identity.key";
const DEFAULT_RELAY_URL: &str = "ws://89.125.209.155:9001";
const DEFAULT_SOCKS5_PROXY: &str = "127.0.0.1:9050";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());
    let config = if args.config.is_some() {
        load_config(&config_path)?
    } else {
        load_or_create_config(&config_path)?
    };
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

fn load_or_create_config(path: &str) -> anyhow::Result<ClientConfig> {
    if Path::new(path).exists() {
        return load_config(path);
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    let config = ClientConfig {
        relay_url: DEFAULT_RELAY_URL.to_string(),
        tor_enabled: false,
        socks5_proxy: Some(DEFAULT_SOCKS5_PROXY.to_string()),
        remote_node_url: Some(String::new()),
        identity_key_path: DEFAULT_IDENTITY_KEY_PATH.to_string(),
    };
    let content = format!(
        "relay_url = \"{}\"\n\
tor_enabled = false\n\
socks5_proxy = \"{}\"\n\
remote_node_url = \"\"\n\
identity_key_path = \"{}\"\n",
        config.relay_url, config.socks5_proxy.as_deref().unwrap_or(""), config.identity_key_path
    );
    fs::write(path, content)?;
    Ok(config)
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
