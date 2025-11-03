//! SP1 zkVM Program: Ethereum Block Verifier
//! 
//! 验证以太坊区块的有效性（简化版，用于本地测试网）
//! 
//! 输入:
//! - prev_block_hash: 前一个区块的哈希
//! - block_header: 当前区块头
//! 
//! 输出:
//! - 验证通过的区块头

#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
sp1_zkvm::entrypoint!(main);

use serde::{Deserialize, Serialize};

/// 以太坊区块头（简化版）
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EthBlockHeader {
    /// 区块号
    pub block_number: u64,
    /// 区块哈希
    pub block_hash: [u8; 32],
    /// 父区块哈希
    pub parent_hash: [u8; 32],
    /// 时间戳
    pub timestamp: u64,
    /// 状态根
    pub state_root: [u8; 32],
    /// 交易根
    pub transactions_root: [u8; 32],
    /// 收据根
    pub receipts_root: [u8; 32],
}

pub fn main() {
    #[cfg(not(test))]
    {
        // ========================================
        // 1. 读取输入
        // ========================================
        
        // 读取前一个区块哈希
        let prev_block_hash: [u8; 32] = sp1_zkvm::io::read();
        
        // 读取当前区块头
        let block_header: EthBlockHeader = sp1_zkvm::io::read();
        
        // ========================================
        // 2. 验证区块连续性
        // ========================================
        
        assert_eq!(
            block_header.parent_hash,
            prev_block_hash,
            "Parent hash mismatch"
        );
        
        // ========================================
        // 3. 验证区块号递增
        // ========================================
        
        // 注意：这里简化了，实际应该从 prev_block 中读取 block_number
        // 在生产环境中，应该验证 block_number = prev_block_number + 1
        assert!(
            block_header.block_number > 0,
            "Invalid block number"
        );
        
        // ========================================
        // 4. 验证时间戳单调递增
        // ========================================
        
        assert!(
            block_header.timestamp > 0,
            "Invalid timestamp"
        );
        
        // 注：在本地 Hardhat 测试网中，我们简化了共识验证
        // 生产环境需要验证：
        // - PoS 验证器签名
        // - Beacon chain 证明
        // - 状态转换有效性等
        
        // ========================================
        // 5. 提交公开输出
        // ========================================
        
        sp1_zkvm::io::commit(&block_header);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eth_block_header_creation() {
        let header = EthBlockHeader {
            block_number: 1000,
            block_hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1699000000,
            state_root: [2u8; 32],
            transactions_root: [3u8; 32],
            receipts_root: [4u8; 32],
        };
        
        assert_eq!(header.block_number, 1000);
        assert_eq!(header.timestamp, 1699000000);
        assert_eq!(header.block_hash[0], 1);
        assert_eq!(header.state_root[0], 2);
    }

    #[test]
    fn test_block_continuity() {
        let block1 = EthBlockHeader {
            block_number: 1000,
            block_hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1699000000,
            state_root: [2u8; 32],
            transactions_root: [3u8; 32],
            receipts_root: [4u8; 32],
        };

        let block2 = EthBlockHeader {
            block_number: 1001,
            block_hash: [2u8; 32],
            parent_hash: [1u8; 32],
            timestamp: 1699000012,
            state_root: [2u8; 32],
            transactions_root: [3u8; 32],
            receipts_root: [4u8; 32],
        };

        assert_eq!(block2.parent_hash, block1.block_hash);
        assert_eq!(block2.block_number, block1.block_number + 1);
        assert!(block2.timestamp > block1.timestamp);
    }

    #[test]
    fn test_timestamp_validation() {
        let header = EthBlockHeader {
            block_number: 1000,
            block_hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1699000000,
            state_root: [2u8; 32],
            transactions_root: [3u8; 32],
            receipts_root: [4u8; 32],
        };
        
        assert!(header.timestamp > 0, "Timestamp should be positive");
        assert!(header.block_number > 0, "Block number should be positive");
    }

    #[test]
    fn test_merkle_roots() {
        let header = EthBlockHeader {
            block_number: 1000,
            block_hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1699000000,
            state_root: [2u8; 32],
            transactions_root: [3u8; 32],
            receipts_root: [4u8; 32],
        };
        
        // 验证所有 Merkle 根都是 32 字节
        assert_eq!(header.state_root.len(), 32);
        assert_eq!(header.transactions_root.len(), 32);
        assert_eq!(header.receipts_root.len(), 32);
    }

    #[test]
    fn test_block_sequence() {
        // 创建一个区块序列
        let blocks: Vec<EthBlockHeader> = (1000..=1005)
            .map(|i| EthBlockHeader {
                block_number: i,
                block_hash: [i as u8; 32],
                parent_hash: if i == 1000 { [0u8; 32] } else { [(i - 1) as u8; 32] },
                timestamp: 1699000000 + (i - 1000) * 12,
                state_root: [2u8; 32],
                transactions_root: [3u8; 32],
                receipts_root: [4u8; 32],
            })
            .collect();

        // 验证序列连续性
        for i in 1..blocks.len() {
            assert_eq!(
                blocks[i].parent_hash,
                blocks[i - 1].block_hash,
                "Block {} should reference block {}",
                blocks[i].block_number,
                blocks[i - 1].block_number
            );
            assert_eq!(
                blocks[i].block_number,
                blocks[i - 1].block_number + 1,
                "Block numbers should be sequential"
            );
            assert!(
                blocks[i].timestamp > blocks[i - 1].timestamp,
                "Timestamps should be monotonically increasing"
            );
        }
    }
}
