//! Solana-Ethereum Bridge Relayer
//! 
//! 监控两条链的区块，生成 SP1 证明，并提交到对应链

mod solana_monitor;
mod ethereum_monitor;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn, error};

use solana_monitor::SolanaMonitor;
use ethereum_monitor::EthereumMonitor;

// ========================================
// 配置
// ========================================

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub solana: SolanaConfig,
    pub ethereum: EthereumConfig,
    pub sp1: Sp1Config,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub min_confirmations: u32,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EthereumConfig {
    pub rpc_url: String,
    pub min_confirmations: u64,
    pub poll_interval_ms: u64,
    pub contract_address: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Sp1Config {
    pub enable_groth16: bool,
    pub prove_timeout_secs: u64,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

// ========================================
// SP1 证明生成器
// ========================================

pub struct Sp1Prover {
    #[allow(dead_code)]
    config: Sp1Config,
    // TODO: Sprint 6 添加 SP1 ProverClient
    // client: ProverClient,
}

impl Sp1Prover {
    pub fn new(config: Sp1Config) -> Self {
        info!("Initializing SP1 prover");
        Self { config }
    }

    pub async fn prove_solana_block(&self, _block_data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Sprint 6 实现 SP1 证明生成
        // 1. 准备输入数据
        // 2. 调用 SP1 SDK
        // 3. 生成 STARK 证明
        // 4. 压缩为 Groth16（如果启用）
        
        info!("Generating proof for Solana block... (mock)");
        Ok(vec![])
    }

    pub async fn prove_eth_block(&self, _block_data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Sprint 6 实现 SP1 证明生成
        
        info!("Generating proof for Ethereum block... (mock)");
        Ok(vec![])
    }
}

// ========================================
// Relayer 主服务
// ========================================

pub struct Relayer {
    solana_monitor: Arc<SolanaMonitor>,
    ethereum_monitor: Arc<EthereumMonitor>,
    #[allow(dead_code)]
    sp1_prover: Arc<Sp1Prover>,
}

impl Relayer {
    pub fn new(config: Config) -> Self {
        Self {
            solana_monitor: Arc::new(SolanaMonitor::new(config.solana)),
            ethereum_monitor: Arc::new(EthereumMonitor::new(config.ethereum)),
            sp1_prover: Arc::new(Sp1Prover::new(config.sp1)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting Solana-Ethereum Bridge Relayer");

        // 启动 Solana 监控器
        let solana_monitor = self.solana_monitor.clone();
        let solana_task = tokio::spawn(async move {
            if let Err(e) = solana_monitor.start().await {
                error!("Solana monitor failed: {:?}", e);
            }
        });

        // 启动 Ethereum 监控器
        let ethereum_monitor = self.ethereum_monitor.clone();
        let ethereum_task = tokio::spawn(async move {
            if let Err(e) = ethereum_monitor.start().await {
                error!("Ethereum monitor failed: {:?}", e);
            }
        });

        // 等待所有任务
        tokio::select! {
            _ = solana_task => {
                warn!("Solana monitor task ended");
            }
            _ = ethereum_task => {
                warn!("Ethereum monitor task ended");
            }
        }

        Ok(())
    }
}

// ========================================
// 主函数
// ========================================

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Solana-Ethereum Bridge Relayer v0.1.0");

    // 加载配置
    let config = Config::load("config.toml")
        .context("Failed to load config.toml")?;

    // 创建并启动 Relayer
    let relayer = Relayer::new(config);
    relayer.start().await?;

    Ok(())
}
