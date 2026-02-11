pub mod mock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxStatus {
    pub tx_id: String,
    pub status: TxState,
    pub confirmations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TxState {
    Pending,
    Confirmed,
    Failed,
}

pub trait Wallet: Send + Sync {
    fn get_balance(&self, pubkey: &str) -> anyhow::Result<String>;
    fn send(
        &self,
        from_pubkey: &str,
        to_pubkey: &str,
        amount_xmr: &str,
        fee_xmr: &str,
    ) -> anyhow::Result<String>;
    fn get_tx_status(&self, tx_id: &str) -> anyhow::Result<TxStatus>;
}
