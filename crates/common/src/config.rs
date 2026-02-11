use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    pub bind_addr: String,
    pub data_dir: String,
    pub dev_fee: DevFeeConfig,
    pub dev_wallet_pubkey: String,
    pub room_price_xmr: String,
    pub username_fee_xmr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub relay_url: String,
    pub tor_enabled: bool,
    pub socks5_proxy: Option<String>,
    pub remote_node_url: Option<String>,
    pub identity_key_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevFeeConfig {
    pub mode: DevFeeMode,
    pub value: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevFeeMode {
    Bps,
    Percent,
}

impl std::fmt::Display for DevFeeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevFeeMode::Bps => write!(f, "bps"),
            DevFeeMode::Percent => write!(f, "percent"),
        }
    }
}

pub fn load_config<T: for<'de> Deserialize<'de>>(path: &str) -> anyhow::Result<T> {
    let content = std::fs::read_to_string(path)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}
