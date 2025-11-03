use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};

use crate::SolanaConfig;
use crate::prover::{Sp1Prover, types::{SolanaBlockData, ValidatorSignature}};

pub struct SolanaMonitor {
    rpc_url: String,
    min_confirmations: u32,
    poll_interval_ms: u64,
    client: RpcClient,
    prover: Arc<Sp1Prover>,
    enable_proving: bool, // 是否启用证明生成（开发时可以关闭）
}

impl SolanaMonitor {
    pub fn new(config: SolanaConfig, prover: Arc<Sp1Prover>) -> Self {
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
            prover,
            enable_proving: false, // 默认关闭，避免在开发时频繁生成证明
        }
    }
    
    /// 启用证明生成
    pub fn enable_proving(&mut self) {
        self.enable_proving = true;
        info!("✅ Solana Monitor: Proof generation enabled");
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
                
                // 获取区块数据
                if let Err(e) = self.process_new_slot(confirmed_slot).await {
                    error!("❌ Failed to process Solana slot {}: {:?}", confirmed_slot, e);
                }
                
                *last_slot = confirmed_slot;
            }
        }
        
        Ok(())
    }
    
    /// 处理新的 Solana slot
    async fn process_new_slot(&self, slot: u64) -> Result<()> {
        // 1. 获取区块数据 (简化版，实际需要更多数据)
        let blockhash = self.get_recent_blockhash()?;
        
        // 2. 构造区块数据
        let block_data = SolanaBlockData {
            slot,
            parent_slot: slot.saturating_sub(1),
            blockhash: self.hash_to_bytes32(&blockhash),
            parent_hash: [0u8; 32], // 简化：实际需要从父区块获取
            signatures: vec![
                // 简化：实际需要从区块获取真实的验证者签名
                ValidatorSignature {
                    validator_pubkey: [1u8; 32],
                    signature: vec![1u8; 64],
                },
            ],
        };
        
        info!("📝 Solana block data prepared: slot {}", slot);
        
        // 3. 生成证明（如果启用）
        if self.enable_proving {
            info!("🔬 Generating ZK proof for Solana slot {}...", slot);
            
            match self.prover.prove_solana_block(&block_data) {
                Ok(proof) => {
                    info!(
                        "✅ Proof generated for Solana slot {}: {} bytes ({})",
                        slot, proof.proof_bytes.len(), 
                        if proof.proof_type == crate::prover::types::ProofType::Groth16 { "Groth16" } else { "STARK" }
                    );
                    
                    // TODO: Task 3 - 提交证明到 Ethereum
                    info!("📤 TODO: Submit proof to Ethereum SolanaUpdater contract");
                }
                Err(e) => {
                    error!("❌ Failed to generate proof for Solana slot {}: {}", slot, e);
                }
            }
        } else {
            warn!("⚠️  Proof generation disabled. Enable with enable_proving()");
        }
        
        Ok(())
    }
    
    /// 获取最近的 blockhash (辅助方法)
    fn get_recent_blockhash(&self) -> Result<String> {
        // 简化实现：实际应该获取特定 slot 的 blockhash
        let hash = self.client.get_latest_blockhash()?;
        Ok(hash.to_string())
    }
    
    /// 将字符串 hash 转换为 [u8; 32]
    fn hash_to_bytes32(&self, hash_str: &str) -> [u8; 32] {
        let mut result = [0u8; 32];
        if let Ok(bytes) = bs58::decode(hash_str).into_vec() {
            let len = bytes.len().min(32);
            result[..len].copy_from_slice(&bytes[..len]);
        }
        result
    }

    /// 获取当前 slot（用于测试）
    #[allow(dead_code)]
    pub fn get_current_slot(&self) -> Result<u64> {
        Ok(self.client.get_slot()?)
    }
}
