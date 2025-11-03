use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};

use crate::EthereumConfig;
use crate::prover::{Sp1Prover, types::EthereumBlockData};

pub struct EthereumMonitor {
    rpc_url: String,
    min_confirmations: u64,
    poll_interval_ms: u64,
    client: Client,
    prover: Arc<Sp1Prover>,
    enable_proving: bool,
}

impl EthereumMonitor {
    pub fn new(config: EthereumConfig, prover: Arc<Sp1Prover>) -> Self {
        info!("Ethereum monitor initialized: {}", config.rpc_url);
        
        Self {
            rpc_url: config.rpc_url,
            min_confirmations: config.min_confirmations,
            poll_interval_ms: config.poll_interval_ms,
            client: Client::new(),
            prover,
            enable_proving: false,
        }
    }
    
    /// 启用证明生成
    pub fn enable_proving(&mut self) {
        self.enable_proving = true;
        info!("✅ Ethereum Monitor: Proof generation enabled");
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
                
                // 处理新区块
                if let Err(e) = self.process_new_block(confirmed_block).await {
                    error!("❌ Failed to process Ethereum block {}: {:?}", confirmed_block, e);
                }
                
                *last_block = confirmed_block;
            }
        }
        
        Ok(())
    }
    
    /// 处理新的 Ethereum 区块
    async fn process_new_block(&self, block_number: u64) -> Result<()> {
        // 1. 获取区块详细数据
        let block_data = self.fetch_block_data(block_number).await?;
        
        info!("📝 Ethereum block data prepared: block {}", block_number);
        
        // 2. 生成证明（如果启用）
        if self.enable_proving {
            info!("🔬 Generating ZK proof for Ethereum block {}...", block_number);
            
            match self.prover.prove_ethereum_block(&block_data) {
                Ok(proof) => {
                    info!(
                        "✅ Proof generated for Ethereum block {}: {} bytes (STARK)",
                        block_number, proof.proof_bytes.len()
                    );
                    
                    // TODO: Task 4 - 提交证明到 Solana
                    info!("📤 TODO: Submit proof to Solana bridge program");
                }
                Err(e) => {
                    error!("❌ Failed to generate proof for Ethereum block {}: {}", block_number, e);
                }
            }
        } else {
            warn!("⚠️  Proof generation disabled. Enable with enable_proving()");
        }
        
        Ok(())
    }
    
    /// 获取区块详细数据
    async fn fetch_block_data(&self, block_number: u64) -> Result<EthereumBlockData> {
        let block_hex = format!("0x{:x}", block_number);
        
        let resp: Value = self.client
            .post(&self.rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_getBlockByNumber",
                "params": [block_hex, false],
                "id": 1
            }))
            .send()
            .await?
            .json()
            .await?;
        
        if let Some(result) = resp.get("result") {
            let hash = self.hex_to_bytes32(result["hash"].as_str().unwrap_or("0x0"))?;
            let parent_hash = self.hex_to_bytes32(result["parentHash"].as_str().unwrap_or("0x0"))?;
            let state_root = self.hex_to_bytes32(result["stateRoot"].as_str().unwrap_or("0x0"))?;
            
            let timestamp_hex = result["timestamp"].as_str().unwrap_or("0x0");
            let timestamp = u64::from_str_radix(&timestamp_hex[2..], 16)?;
            
            Ok(EthereumBlockData {
                number: block_number,
                hash,
                parent_hash,
                timestamp,
                state_root,
            })
        } else {
            Err(anyhow::anyhow!("Failed to fetch block {}", block_number))
        }
    }
    
    /// 将 hex 字符串转换为 [u8; 32]
    fn hex_to_bytes32(&self, hex: &str) -> Result<[u8; 32]> {
        let hex = hex.trim_start_matches("0x");
        let mut result = [0u8; 32];
        
        if hex.len() >= 64 {
            for i in 0..32 {
                result[i] = u8::from_str_radix(&hex[i*2..i*2+2], 16)?;
            }
        }
        
        Ok(result)
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
