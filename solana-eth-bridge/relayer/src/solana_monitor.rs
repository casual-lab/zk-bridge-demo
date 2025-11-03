use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use tokio::time::{sleep, Duration};
use tracing::{info, error};

use crate::SolanaConfig;

pub struct SolanaMonitor {
    rpc_url: String,
    min_confirmations: u32,
    poll_interval_ms: u64,
    client: RpcClient,
}

impl SolanaMonitor {
    pub fn new(config: SolanaConfig) -> Self {
        let client = RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );
        
        info!("Solana monitor initialized: {}", config.rpc_url);
        
        Self {
            rpc_url: config.rpc_url,
            min_confirmations: config.min_confirmations,
            poll_interval_ms: config.poll_interval_ms,
            client,
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting Solana monitor on {}", self.rpc_url);
        
        let mut last_slot = 0u64;
        
        loop {
            match self.check_new_blocks(&mut last_slot).await {
                Ok(_) => {}
                Err(e) => {
                    error!("❌ Solana monitor error: {:?}", e);
                }
            }
            
            sleep(Duration::from_millis(self.poll_interval_ms)).await;
        }
    }

    async fn check_new_blocks(&self, last_slot: &mut u64) -> Result<()> {
        // 获取当前 slot
        let current_slot = self.client.get_slot()?;
        
        if current_slot > *last_slot {
            // 计算确认的 slot（减去确认深度）
            let confirmed_slot = current_slot.saturating_sub(self.min_confirmations as u64);
            
            if confirmed_slot > *last_slot {
                info!(
                    "📦 New confirmed Solana slot: {} (current: {}, confirmations: {})",
                    confirmed_slot, current_slot, self.min_confirmations
                );
                
                // TODO: Sprint 5 - 获取区块数据和签名
                // TODO: Sprint 6 - 触发 SP1 证明生成
                
                *last_slot = confirmed_slot;
            }
        }
        
        Ok(())
    }

    /// 获取当前 slot（用于测试）
    #[allow(dead_code)]
    pub fn get_current_slot(&self) -> Result<u64> {
        Ok(self.client.get_slot()?)
    }
}
