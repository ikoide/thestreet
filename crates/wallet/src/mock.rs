use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::{TxState, TxStatus, Wallet};

#[derive(Clone, Default)]
pub struct MockWallet {
    balances: Arc<Mutex<HashMap<String, f64>>>,
    txs: Arc<Mutex<HashMap<String, TxStatus>>>,
}

impl MockWallet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn credit(&self, pubkey: &str, amount: f64) {
        let mut balances = self.balances.lock().expect("balances lock");
        let entry = balances.entry(pubkey.to_string()).or_insert(0.0);
        *entry += amount;
    }
}

impl Wallet for MockWallet {
    fn get_balance(&self, pubkey: &str) -> anyhow::Result<String> {
        let balances = self.balances.lock().expect("balances lock");
        let balance = balances.get(pubkey).copied().unwrap_or(0.0);
        Ok(format!("{:.8}", balance))
    }

    fn send(
        &self,
        from_pubkey: &str,
        to_pubkey: &str,
        amount_xmr: &str,
        fee_xmr: &str,
    ) -> anyhow::Result<String> {
        let amount: f64 = amount_xmr.parse().unwrap_or(0.0);
        let fee: f64 = fee_xmr.parse().unwrap_or(0.0);
        let total = amount + fee;
        let mut balances = self.balances.lock().expect("balances lock");
        let from_balance = balances.get(from_pubkey).copied().unwrap_or(0.0);
        if from_balance < total {
            anyhow::bail!("insufficient funds")
        }
        balances.insert(from_pubkey.to_string(), from_balance - total);
        let to_balance = balances.get(to_pubkey).copied().unwrap_or(0.0);
        balances.insert(to_pubkey.to_string(), to_balance + amount);

        let tx_id = Uuid::new_v4().to_string();
        let status = TxStatus {
            tx_id: tx_id.clone(),
            status: TxState::Pending,
            confirmations: 0,
        };
        let mut txs = self.txs.lock().expect("txs lock");
        txs.insert(tx_id.clone(), status);
        Ok(tx_id)
    }

    fn get_tx_status(&self, tx_id: &str) -> anyhow::Result<TxStatus> {
        let txs = self.txs.lock().expect("txs lock");
        let status = txs.get(tx_id).cloned().unwrap_or(TxStatus {
            tx_id: tx_id.to_string(),
            status: TxState::Failed,
            confirmations: 0,
        });
        Ok(status)
    }
}
