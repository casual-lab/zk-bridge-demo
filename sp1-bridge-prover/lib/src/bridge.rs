use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};

// Solidity 结构体定义，用于 EVM 链上验证
sol! {
    /// 跨链订单验证的公开输入/输出
    struct BridgeProofPublicValues {
        /// 订单 ID
        uint64 orderId;
        /// 源链 ID (0 = Solana, 1 = EVM)
        uint8 sourceChain;
        /// 目标链 ID
        uint8 targetChain;
        /// 代币地址（32 字节，适配 Solana 或 EVM）
        bytes32 token;
        /// 转账金额
        uint256 amount;
        /// 接收者地址（32 字节）
        bytes32 recipient;
        /// 订单状态哈希（防止篡改）
        bytes32 stateRoot;
        /// 时间戳/区块号
        uint64 timestamp;
    }
}

/// Rust 原生的订单数据结构
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferOrder {
    pub order_id: u64,
    pub user: [u8; 32],           // Solana pubkey 或 EVM address (左填充)
    pub source_chain: u8,          // 0 = Solana, 1 = EVM
    pub token: [u8; 32],           // Token address
    pub amount: u64,
    pub recipient: [u8; 32],       // 接收者地址
    pub relayer_fee: u64,
    pub created_at: u64,           // slot 或 block number
    pub status: OrderStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Pending,
    Completed,
}

/// Merkle 证明验证
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf: [u8; 32],
    pub proof: Vec<[u8; 32]>,
    pub root: [u8; 32],
}

impl MerkleProof {
    /// 验证 Merkle 证明
    pub fn verify(&self) -> bool {
        let mut current = self.leaf;
        
        for sibling in &self.proof {
            current = if current <= *sibling {
                hash_pair(&current, sibling)
            } else {
                hash_pair(sibling, &current)
            };
        }
        
        current == self.root
    }
}

/// 简单的哈希函数（使用 sha256）
fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(a);
    hasher.update(b);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// 计算订单的哈希
pub fn hash_order(order: &TransferOrder) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    
    hasher.update(&order.order_id.to_le_bytes());
    hasher.update(&order.user);
    hasher.update(&[order.source_chain]);
    hasher.update(&order.token);
    hasher.update(&order.amount.to_le_bytes());
    hasher.update(&order.recipient);
    hasher.update(&order.relayer_fee.to_le_bytes());
    hasher.update(&order.created_at.to_le_bytes());
    
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_proof_verify() {
        // 创建一个简单的 Merkle 树测试
        let leaf = [1u8; 32];
        let sibling = [2u8; 32];
        let root = hash_pair(&leaf, &sibling);
        
        let proof = MerkleProof {
            leaf,
            proof: vec![sibling],
            root,
        };
        
        assert!(proof.verify());
    }
    
    #[test]
    fn test_merkle_proof_invalid() {
        let leaf = [1u8; 32];
        let sibling = [2u8; 32];
        let wrong_root = [3u8; 32];
        
        let proof = MerkleProof {
            leaf,
            proof: vec![sibling],
            root: wrong_root,
        };
        
        assert!(!proof.verify());
    }
    
    #[test]
    fn test_hash_order() {
        let order = TransferOrder {
            order_id: 1,
            user: [0x01; 32],
            source_chain: 0,
            token: [0x02; 32],
            amount: 1_000_000,
            recipient: [0x03; 32],
            relayer_fee: 1_000,
            created_at: 100,
            status: OrderStatus::Pending,
        };
        
        let hash = hash_order(&order);
        assert_ne!(hash, [0u8; 32]);
        
        // 相同的订单应该产生相同的哈希
        let hash2 = hash_order(&order);
        assert_eq!(hash, hash2);
    }
}
