use serde::{Deserialize, Serialize};

/// Solana 区块数据（用于生成证明）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaBlockData {
    pub slot: u64,
    pub parent_slot: u64,
    pub blockhash: [u8; 32],
    pub parent_hash: [u8; 32],
    /// 验证者签名列表
    pub signatures: Vec<ValidatorSignature>,
}

/// 验证者签名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSignature {
    pub validator_pubkey: [u8; 32],
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>, // 64 bytes Ed25519 signature
}

/// Ethereum 区块数据（用于生成证明）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumBlockData {
    pub number: u64,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub timestamp: u64,
    pub state_root: [u8; 32],
}

/// 证明结果
#[derive(Debug, Clone)]
pub struct ProofResult {
    /// 证明数据
    pub proof_bytes: Vec<u8>,
    /// 公开输入（验证时需要）
    pub public_values: Vec<u8>,
    /// 证明类型
    pub proof_type: ProofType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofType {
    /// STARK 证明（用于 Solana 链上验证）
    Stark,
    /// Groth16 证明（用于 Ethereum 链上验证，经过压缩）
    Groth16,
}

/// 证明生成错误
#[derive(Debug, thiserror::Error)]
pub enum ProverError {
    #[error("证明生成失败: {0}")]
    ProofGeneration(String),
    
    #[error("证明压缩失败: {0}")]
    ProofCompression(String),
    
    #[error("ELF 文件加载失败: {0}")]
    ElfLoad(String),
    
    #[error("序列化错误: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("SP1 SDK 错误: {0}")]
    Sp1Error(String),
}
