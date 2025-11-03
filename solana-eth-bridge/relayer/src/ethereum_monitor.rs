use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};
use tracing::{info, error};

use crate::EthereumConfig;

pub struct EthereumMonitor {
    rpc_url: String,
    min_confirmations: u64,
    poll_interval_ms: u64,
    client: Client,
}

impl EthereumMonitor {
    pub fn new(config: EthereumConfig) -> Self {
        info!("Ethereum monitor initialized: {}", config.rpc_url);
        
        Self {
            rpc_url: config.rpc_url,
            min_confirmations: config.min_confirmations,
            poll_interval_ms: config.poll_interval_ms,
            client: Client::new(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting Ethereum monitor on {}", self.rpc_url);
        
        let mut last_block = 0u64;
        
        loop {
            match self.check_new_blocks(&mut last_block).await {
                Ok(_) => {}
                Err(e) => {
                    error!("❌ Ethereum monitor error: {:?}", e);
                }
            }
            
            sleep(Duration::from_millis(self.poll_interval_ms)).await;
        }
    }

    async fn check_new_blocks(&self, last_block: &mut u64) -> Result<()> {
        // 调用 eth_blockNumber
        let resp: Value = self.client
            .post(&self.rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            }))
            .send()
            .await?
            .json()
            .await?;
        
        if let Some(result) = resp.get("result") {
            let block_hex = result.as_str().unwrap_or("0x0");
            let current_block = u64::from_str_radix(&block_hex[2..], 16)?;
            
            let confirmed_block = current_block.saturating_sub(self.min_confirmations);
            
            if confirmed_block > *last_block {
                info!(
                    "📦 New confirmed Ethereum block: {} (current: {}, confirmations: {})",
                    confirmed_block, current_block, self.min_confirmations
                );
                
                // TODO: Sprint 5 - 获取区块头数据
                // TODO: Sprint 6 - 触发 SP1 证明生成
                
                *last_block = confirmed_block;
            }
        }
        
        Ok(())
    }

    /// 获取当前区块号（用于测试）
    #[allow(dead_code)]
    pub async fn get_current_block(&self) -> Result<u64> {
        let resp: Value = self.client
            .post(&self.rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            }))
            .send()
            .await?
            .json()
            .await?;
        
        if let Some(result) = resp.get("result") {
            let block_hex = result.as_str().unwrap_or("0x0");
            Ok(u64::from_str_radix(&block_hex[2..], 16)?)
        } else {
            Ok(0)
        }
    }
}
