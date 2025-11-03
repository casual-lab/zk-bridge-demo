//! Solana-Ethereum Bridge Relayer
//! 
//! 监控两条链的区块，生成 SP1 证明，并提交到对应链

mod solana_monitor;
mod ethereum_monitor;
mod prover;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn, error};

use solana_monitor::SolanaMonitor;
use ethereum_monitor::EthereumMonitor;
use prover::Sp1Prover;

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
// Relayer 主服务
// ========================================

pub struct Relayer {
    solana_monitor: Arc<SolanaMonitor>,
    ethereum_monitor: Arc<EthereumMonitor>,
}

impl Relayer {
    pub fn new(config: Config) -> Self {
        info!("Initializing Relayer...");
        
        // 创建 SP1 Prover (共享)
        let prover = Arc::new(Sp1Prover::new());
        
        // 创建监控器
        let solana_monitor = Arc::new(SolanaMonitor::new(config.solana, prover.clone()));
        let ethereum_monitor = Arc::new(EthereumMonitor::new(config.ethereum, prover));
        
        info!("✅ Relayer initialized");
        
        Self {
            solana_monitor,
            ethereum_monitor,
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
