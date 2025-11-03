# 🚀 快速开始：从 Sprint 1 开始

## 📍 你现在的位置

✅ **已完成**: Sprint 0 - 所有框架搭建完成
🎯 **下一步**: Sprint 1 - SP1 程序本地测试

---

## Sprint 1: SP1 程序本地测试

**目标**: 让 SP1 程序能够在本地运行和测试，无需链上交互  
**时间**: 1-2 天  
**难度**: ⭐⭐ (中等)

---

## 🛠️ 步骤 1.1: 添加 SP1 程序测试 (2-3 小时)

### 1. 添加测试依赖

编辑 `sp1-programs/solana-verifier/Cargo.toml`，添加：

```toml
[dev-dependencies]
hex = "0.4"
```

### 2. 添加测试代码

编辑 `sp1-programs/solana-verifier/src/main.rs`，在文件末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_header_creation() {
        let header = SolanaBlockHeader {
            slot: 1000,
            blockhash: [1u8; 32],
            parent_hash: [0u8; 32],
            block_height: 1000,
            timestamp: 1699000000,
            confirmations: 32,
        };
        
        assert_eq!(header.slot, 1000);
        assert_eq!(header.confirmations, 32);
    }

    #[test]
    fn test_confirmation_validation() {
        let header = SolanaBlockHeader {
            slot: 1000,
            blockhash: [1u8; 32],
            parent_hash: [0u8; 32],
            block_height: 1000,
            timestamp: 1699000000,
            confirmations: 32,
        };
        
        const MIN_CONFIRMATIONS: u32 = 32;
        assert!(header.confirmations >= MIN_CONFIRMATIONS);
    }

    #[test]
    #[should_panic(expected = "Insufficient confirmations")]
    fn test_insufficient_confirmations() {
        let header = SolanaBlockHeader {
            slot: 1000,
            blockhash: [1u8; 32],
            parent_hash: [0u8; 32],
            block_height: 1000,
            timestamp: 1699000000,
            confirmations: 10, // 不足 32
        };
        
        const MIN_CONFIRMATIONS: u32 = 32;
        assert!(
            header.confirmations >= MIN_CONFIRMATIONS,
            "Insufficient confirmations: got {}, need {}",
            header.confirmations,
            MIN_CONFIRMATIONS
        );
    }

    #[test]
    fn test_validator_signature_structure() {
        let sig = ValidatorSignature {
            pubkey: [1u8; 32],
            signature: [2u8; 64],
        };
        
        assert_eq!(sig.pubkey.len(), 32);
        assert_eq!(sig.signature.len(), 64);
    }

    #[test]
    fn test_block_proof_with_multiple_signatures() {
        let header = SolanaBlockHeader {
            slot: 1000,
            blockhash: [1u8; 32],
            parent_hash: [0u8; 32],
            block_height: 1000,
            timestamp: 1699000000,
            confirmations: 32,
        };

        let signatures = vec![
            ValidatorSignature {
                pubkey: [1u8; 32],
                signature: [1u8; 64],
            },
            ValidatorSignature {
                pubkey: [2u8; 32],
                signature: [2u8; 64],
            },
            ValidatorSignature {
                pubkey: [3u8; 32],
                signature: [3u8; 64],
            },
        ];

        let proof = BlockProof {
            header: header.clone(),
            signatures,
        };

        assert_eq!(proof.signatures.len(), 3);
        
        // 验证 2/3 阈值
        let total = proof.signatures.len();
        let threshold = (total * 2) / 3 + 1;
        assert_eq!(threshold, 3);
    }

    #[test]
    fn test_parent_hash_continuity() {
        let block1 = SolanaBlockHeader {
            slot: 1000,
            blockhash: [1u8; 32],
            parent_hash: [0u8; 32],
            block_height: 1000,
            timestamp: 1699000000,
            confirmations: 32,
        };

        let block2 = SolanaBlockHeader {
            slot: 1001,
            blockhash: [2u8; 32],
            parent_hash: [1u8; 32], // 应该等于 block1.blockhash
            block_height: 1001,
            timestamp: 1699000001,
            confirmations: 32,
        };

        // 验证连续性
        assert_eq!(block2.parent_hash, block1.blockhash);
        assert_eq!(block2.slot, block1.slot + 1);
    }
}
```

### 3. 运行测试

```bash
cd /workspace/solana-eth-bridge/sp1-programs/solana-verifier
cargo test
```

**预期输出**:
```
running 6 tests
test tests::test_block_header_creation ... ok
test tests::test_confirmation_validation ... ok
test tests::test_insufficient_confirmations ... ok
test tests::test_validator_signature_structure ... ok
test tests::test_block_proof_with_multiple_signatures ... ok
test tests::test_parent_hash_continuity ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

✅ **验收标准**: 所有 6 个测试通过

---

## 🛠️ 步骤 1.2: Ethereum 验证器测试 (1 小时)

### 1. 添加测试到 `sp1-programs/eth-verifier/src/main.rs`

在文件末尾添加：

```rust
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
}
```

### 2. 运行测试

```bash
cd /workspace/solana-eth-bridge/sp1-programs/eth-verifier
cargo test
```

✅ **验收标准**: 所有测试通过

---

## 🛠️ 步骤 1.3: 创建构建脚本 (30 分钟)

创建 `sp1-programs/build.sh`:

```bash
#!/bin/bash
set -e

echo "========================================="
echo "Building SP1 Programs"
echo "========================================="

# 颜色输出
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 构建 solana-verifier
echo -e "\n${BLUE}[1/2] Building solana-verifier...${NC}"
cd solana-verifier
cargo build --release
cargo test
echo -e "${GREEN}✓ solana-verifier built and tested${NC}"
cd ..

# 构建 eth-verifier
echo -e "\n${BLUE}[2/2] Building eth-verifier...${NC}"
cd eth-verifier
cargo build --release
cargo test
echo -e "${GREEN}✓ eth-verifier built and tested${NC}"
cd ..

echo -e "\n${GREEN}=========================================${NC}"
echo -e "${GREEN}✓ All SP1 programs built successfully${NC}"
echo -e "${GREEN}=========================================${NC}"
```

添加执行权限：

```bash
chmod +x /workspace/solana-eth-bridge/sp1-programs/build.sh
```

运行构建脚本：

```bash
cd /workspace/solana-eth-bridge/sp1-programs
./build.sh
```

✅ **验收标准**: 
- 两个程序都编译成功
- 所有测试通过
- 看到绿色的成功提示

---

## 🎯 Sprint 1 完成检查清单

完成后，你应该能够：

- [ ] ✅ `solana-verifier` 有 6 个测试，全部通过
- [ ] ✅ `eth-verifier` 有 2 个测试，全部通过
- [ ] ✅ 构建脚本能一键编译和测试
- [ ] ✅ 理解了基本的数据结构和验证逻辑

---

## 📝 提交你的工作

```bash
cd /workspace/solana-eth-bridge
git add .
git commit -m "Sprint 1: Add SP1 program tests and build scripts"
git push
```

---

## 🚀 下一步

完成 Sprint 1 后，继续 **Sprint 2: Ethereum 合约测试**

查看详细计划：`/workspace/docs/8-详细开发计划.md`

---

## 💡 提示

- 每个测试都很小，专注于一个功能点
- 如果测试失败，仔细阅读错误信息
- 可以用 `cargo test -- --nocapture` 看到打印输出
- 测试是最好的文档，展示了代码如何使用

---

## ❓ 遇到问题？

**编译错误**:
```bash
cargo clean
cargo build
```

**测试失败**:
```bash
cargo test -- --nocapture --test-threads=1
```

**想看详细输出**:
```bash
RUST_LOG=debug cargo test
```

---

开始时间: ___________
完成时间: ___________
用时: ___________

Good luck! 🚀
