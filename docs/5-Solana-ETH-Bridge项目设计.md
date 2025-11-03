# Solana ↔ Ethereum 跨链桥 with SP1 zkVM

## 项目概述

使用 SP1 zkVM 实现 Solana 和 Ethereum 之间的**零信任双向跨链桥**，在本地测试环境完成 POC 验证。

### 技术栈

| 组件 | 技术选择 | 备注 |
|------|---------|------|
| **证明系统** | SP1 zkVM (STARK + Groth16) | 替代 deVirgo |
| **Solana 侧** | Anchor Framework | 智能合约 |
| **Ethereum 侧** | Hardhat + Solidity | 智能合约 |
| **Relayer** | Rust + Tokio | 双向中继 |
| **测试环境** | solana-test-validator + Hardhat Network | 本地测试 |

---

## 系统架构

### 双向桥流程

```
┌────────────────────────────────────────────────────────────┐
│                    Solana (本地测试网)                       │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Bridge Program (Anchor):                                 │
│  ┌──────────────────────────────────────────────────┐    │
│  │ pub struct BridgeState {                          │    │
│  │   pub eth_headers: Vec<EthBlockHeader>,  // 存储ETH块│    │
│  │   pub last_eth_block: u64,               // 最后同步│    │
│  │   pub admin: Pubkey,                              │    │
│  │ }                                                 │    │
│  │                                                   │    │
│  │ pub fn verify_eth_block(                         │    │
│  │   proof: Vec<u8>,        // SP1 Groth16 证明    │    │
│  │   block_header: EthBlockHeader,                  │    │
│  │ ) -> Result<()>                                  │    │
│  └──────────────────────────────────────────────────┘    │
│                                                            │
│  Token Vault (PDA):                                       │
│  - 锁定 SOL/SPL 代币                                       │
│  - 接收跨链消息后释放                                       │
│                                                            │
└──────────────────────┬─────────────────────────────────────┘
                       │
                       │  方向 1: Solana → ETH
                       │  ─────────────────►
                       │
                       │  方向 2: ETH → Solana
                       │  ◄─────────────────
                       │
                       ▼
┌────────────────────────────────────────────────────────────┐
│                    Relayer (Rust)                          │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Solana Monitor:                                          │
│  - 监听 Solana 区块和交易                                   │
│  - 获取区块签名数据                                         │
│  - 触发 SP1 证明生成                                        │
│                                                            │
│  Ethereum Monitor:                                        │
│  - 监听 Ethereum 区块                                      │
│  - 获取区块头数据                                          │
│  - 触发 SP1 证明生成                                        │
│                                                            │
│  SP1 Prover:                                              │
│  - Guest Program 1: verify_solana_block()                 │
│  - Guest Program 2: verify_eth_block()                    │
│  - 生成 Groth16 压缩证明                                    │
│                                                            │
│  Transaction Submitter:                                   │
│  - 提交证明到对应链                                         │
│  - 重试和错误处理                                          │
│                                                            │
└──────────────────────┬─────────────────────────────────────┘
                       │
                       ▼
┌────────────────────────────────────────────────────────────┐
│                 Ethereum (Hardhat 本地网络)                 │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  SolanaUpdater Contract:                                  │
│  ┌──────────────────────────────────────────────────┐    │
│  │ mapping(uint64 => SolanaBlockHeader) public      │    │
│  │     solanaHeaders;                                │    │
│  │                                                   │    │
│  │ struct SolanaBlockHeader {                        │    │
│  │   uint64 slot;                                    │    │
│  │   bytes32 blockhash;                              │    │
│  │   bytes32 parentHash;                             │    │
│  │   uint64 timestamp;                               │    │
│  │ }                                                 │    │
│  │                                                   │    │
│  │ function updateSolanaBlock(                       │    │
│  │   bytes calldata proof,                          │    │
│  │   SolanaBlockHeader calldata header              │    │
│  │ ) external                                       │    │
│  └──────────────────────────────────────────────────┘    │
│                                                            │
│  Token Bridge Contract:                                   │
│  - 锁定/铸造 ETH/ERC20                                     │
│  - 验证跨链消息                                            │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

---

## 核心组件设计

### 1. SP1 Guest Program - Solana 验证

#### 文件结构
```
solana-eth-bridge/
├── sp1-programs/
│   ├── solana-verifier/      # Solana → ETH
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   └── eth-verifier/          # ETH → Solana
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
```

#### Solana 验证逻辑

```rust
// sp1-programs/solana-verifier/src/main.rs
#![no_main]
sp1_zkvm::entrypoint!(main);

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SolanaBlockHeader {
    pub slot: u64,
    pub blockhash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub block_height: u64,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SolanaBlockProof {
    pub header: SolanaBlockHeader,
    pub signatures: Vec<ValidatorSignature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ValidatorSignature {
    pub pubkey: [u8; 32],
    pub signature: [u8; 64],
}

pub fn main() {
    // 1. 读取输入
    let prev_blockhash: [u8; 32] = sp1_zkvm::io::read();
    let block_proof: SolanaBlockProof = sp1_zkvm::io::read();
    
    // 2. 验证区块连续性
    assert_eq!(
        block_proof.header.parent_hash,
        prev_blockhash,
        "Parent hash mismatch"
    );
    
    // 3. 验证签名（简化版：Solana 使用 Tower BFT）
    // 实际需要验证 >2/3 的 stake-weighted 签名
    verify_tower_bft_signatures(&block_proof);
    
    // 4. 提交公开输出
    sp1_zkvm::io::commit(&block_proof.header);
}

fn verify_tower_bft_signatures(proof: &SolanaBlockProof) {
    // 构造签名消息
    let message = create_block_sign_message(&proof.header);
    
    let mut valid_count = 0;
    
    for sig in &proof.signatures {
        // 使用 SP1 Ed25519 预编译
        let valid = sp1_zkvm::precompiles::ed25519::verify(
            &sig.pubkey,
            &message,
            &sig.signature,
        );
        
        if valid {
            valid_count += 1;
        }
    }
    
    // Solana 需要 >2/3 的验证器签名
    // 简化: 假设所有验证器权重相同
    let threshold = (proof.signatures.len() * 2) / 3 + 1;
    assert!(
        valid_count >= threshold,
        "Insufficient signatures: {} < {}",
        valid_count,
        threshold
    );
}

fn create_block_sign_message(header: &SolanaBlockHeader) -> Vec<u8> {
    // Solana 的区块签名消息格式
    let mut message = Vec::new();
    message.extend_from_slice(&header.slot.to_le_bytes());
    message.extend_from_slice(&header.blockhash);
    message.extend_from_slice(&header.parent_hash);
    message
}
```

### 2. SP1 Guest Program - Ethereum 验证

```rust
// sp1-programs/eth-verifier/src/main.rs
#![no_main]
sp1_zkvm::entrypoint!(main);

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EthBlockHeader {
    pub number: u64,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub transactions_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub timestamp: u64,
    pub difficulty: u64,  // 或 PoS 后的随机数
}

pub fn main() {
    // 1. 读取输入
    let prev_block: EthBlockHeader = sp1_zkvm::io::read();
    let new_block: EthBlockHeader = sp1_zkvm::io::read();
    
    // 2. 验证区块连续性
    assert_eq!(
        new_block.parent_hash,
        prev_block.hash,
        "Parent hash mismatch"
    );
    
    assert_eq!(
        new_block.number,
        prev_block.number + 1,
        "Block number not continuous"
    );
    
    // 3. 验证时间戳递增
    assert!(
        new_block.timestamp > prev_block.timestamp,
        "Timestamp must increase"
    );
    
    // 4. 对于本地 Hardhat 测试，不需要验证 PoS 签名
    // 生产环境需要验证 Beacon Chain 的 BLS 签名
    
    // 5. 提交公开输出
    sp1_zkvm::io::commit(&new_block);
}
```

---

### 3. Solana Bridge Program (Anchor)

#### 项目结构
```
solana-eth-bridge/
├── programs/
│   └── bridge/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── state.rs
│           └── instructions/
│               ├── mod.rs
│               ├── initialize.rs
│               └── verify_eth_block.rs
```

#### 核心代码

```rust
// programs/bridge/src/lib.rs
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;

declare_id!("BridgeProgramXXXXXXXXXXXXXXXXXXXXXXXXXXXX");

#[program]
pub mod bridge {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        initial_eth_block: EthBlockHeader,
    ) -> Result<()> {
        let bridge_state = &mut ctx.accounts.bridge_state;
        bridge_state.admin = ctx.accounts.admin.key();
        bridge_state.last_eth_block = initial_eth_block.number;
        bridge_state.eth_headers.push(initial_eth_block);
        Ok(())
    }

    pub fn verify_eth_block(
        ctx: Context<VerifyEthBlock>,
        proof: Vec<u8>,
        new_block: EthBlockHeader,
    ) -> Result<()> {
        let bridge_state = &mut ctx.accounts.bridge_state;
        
        // 1. 检查区块连续性
        require!(
            new_block.number == bridge_state.last_eth_block + 1,
            BridgeError::InvalidBlockNumber
        );
        
        // 2. 验证 SP1 Groth16 证明
        // 注意: Solana 上验证 Groth16 需要特殊处理
        // 可以使用预编译或离线验证
        verify_sp1_proof(&proof, &new_block)?;
        
        // 3. 更新状态
        bridge_state.eth_headers.push(new_block.clone());
        bridge_state.last_eth_block = new_block.number;
        
        emit!(EthBlockVerified {
            block_number: new_block.number,
            block_hash: new_block.hash,
        });
        
        Ok(())
    }
    
    pub fn lock_tokens(
        ctx: Context<LockTokens>,
        amount: u64,
        eth_recipient: [u8; 20],
    ) -> Result<()> {
        // 锁定 SOL 或 SPL 代币
        // 触发跨链事件
        emit!(TokensLocked {
            user: ctx.accounts.user.key(),
            amount,
            eth_recipient,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + BridgeState::INIT_SPACE,
        seeds = [b"bridge"],
        bump
    )]
    pub bridge_state: Account<'info, BridgeState>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyEthBlock<'info> {
    #[account(
        mut,
        seeds = [b"bridge"],
        bump,
    )]
    pub bridge_state: Account<'info, BridgeState>,
    
    pub authority: Signer<'info>,
}

#[account]
pub struct BridgeState {
    pub admin: Pubkey,
    pub last_eth_block: u64,
    pub eth_headers: Vec<EthBlockHeader>,
}

impl BridgeState {
    pub const INIT_SPACE: usize = 32 + 8 + (64 * 100); // 存储最近 100 个 ETH 块
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct EthBlockHeader {
    pub number: u64,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub timestamp: u64,
}

#[event]
pub struct EthBlockVerified {
    pub block_number: u64,
    pub block_hash: [u8; 32],
}

#[event]
pub struct TokensLocked {
    pub user: Pubkey,
    pub amount: u64,
    pub eth_recipient: [u8; 20],
}

#[error_code]
pub enum BridgeError {
    #[msg("Invalid block number")]
    InvalidBlockNumber,
    #[msg("Invalid proof")]
    InvalidProof,
}

// SP1 证明验证（简化版）
fn verify_sp1_proof(
    proof: &[u8],
    block: &EthBlockHeader,
) -> Result<()> {
    // 在 Solana 上验证 Groth16 证明的选项:
    // 1. 使用 Solana 的 ed25519 预编译 (不直接支持 Groth16)
    // 2. 离线验证 + 可信中继
    // 3. 等待 Solana 支持 Groth16 预编译
    
    // 暂时: 检查证明非空 (生产环境需要完整验证)
    require!(proof.len() > 0, BridgeError::InvalidProof);
    Ok(())
}
```

---

### 4. Ethereum Updater Contract

```solidity
// contracts/SolanaUpdater.sol
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface ISP1Verifier {
    function verifyProof(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external view;
}

contract SolanaUpdater {
    // SP1 验证器
    ISP1Verifier public immutable sp1Verifier;
    bytes32 public immutable solanaProgramVKey;
    
    // Solana 区块头存储
    mapping(uint64 => SolanaBlockHeader) public solanaHeaders;
    uint64 public lastSolanaSlot;
    
    struct SolanaBlockHeader {
        uint64 slot;
        bytes32 blockhash;
        bytes32 parentHash;
        uint64 blockHeight;
        int64 timestamp;
    }
    
    event SolanaBlockUpdated(
        uint64 indexed slot,
        bytes32 indexed blockhash
    );
    
    constructor(
        address _sp1Verifier,
        bytes32 _solanaProgramVKey,
        SolanaBlockHeader memory genesisBlock
    ) {
        sp1Verifier = ISP1Verifier(_sp1Verifier);
        solanaProgramVKey = _solanaProgramVKey;
        
        // 初始化创世块
        solanaHeaders[genesisBlock.slot] = genesisBlock;
        lastSolanaSlot = genesisBlock.slot;
    }
    
    /**
     * @notice 更新 Solana 区块头
     */
    function updateSolanaBlock(
        bytes calldata proof,
        SolanaBlockHeader calldata newBlock
    ) external {
        // 1. 检查父块存在
        require(
            solanaHeaders[newBlock.slot - 1].slot != 0,
            "Parent block not found"
        );
        
        // 2. 检查 slot 连续性
        require(
            newBlock.slot == lastSolanaSlot + 1,
            "Slot must be sequential"
        );
        
        // 3. 验证父哈希
        require(
            newBlock.parentHash == solanaHeaders[lastSolanaSlot].blockhash,
            "Parent hash mismatch"
        );
        
        // 4. 准备公开输入
        bytes memory publicValues = abi.encode(newBlock);
        
        // 5. 验证 SP1 证明
        sp1Verifier.verifyProof(
            solanaProgramVKey,
            publicValues,
            proof
        );
        
        // 6. 更新状态
        solanaHeaders[newBlock.slot] = newBlock;
        lastSolanaSlot = newBlock.slot;
        
        emit SolanaBlockUpdated(newBlock.slot, newBlock.blockhash);
    }
    
    /**
     * @notice 获取 Solana 区块头
     */
    function getSolanaBlock(uint64 slot)
        external
        view
        returns (SolanaBlockHeader memory)
    {
        require(
            solanaHeaders[slot].slot != 0,
            "Block not found"
        );
        return solanaHeaders[slot];
    }
}
```

---

### 5. Relayer

```rust
// relayer/src/main.rs
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::Keypair,
};
use ethers::prelude::*;
use sp1_sdk::{ProverClient, SP1Stdin};
use tokio;

const SOLANA_VERIFIER_ELF: &[u8] = 
    include_bytes!("../../sp1-programs/solana-verifier/elf/riscv32im-succinct-zkvm-elf");
const ETH_VERIFIER_ELF: &[u8] = 
    include_bytes!("../../sp1-programs/eth-verifier/elf/riscv32im-succinct-zkvm-elf");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌉 Starting Solana <-> Ethereum Bridge Relayer");
    
    // 初始化客户端
    let solana_client = RpcClient::new_with_commitment(
        "http://localhost:8899".to_string(),
        CommitmentConfig::confirmed(),
    );
    
    let eth_provider = Provider::<Http>::try_from("http://localhost:8545")?;
    
    // 启动双向监听
    let solana_to_eth = tokio::spawn(async move {
        relay_solana_to_eth(solana_client, eth_provider).await
    });
    
    let eth_to_solana = tokio::spawn(async move {
        relay_eth_to_solana().await
    });
    
    tokio::try_join!(solana_to_eth, eth_to_solana)?;
    
    Ok(())
}

/// Solana → Ethereum 中继
async fn relay_solana_to_eth(
    solana_client: RpcClient,
    eth_provider: Provider<Http>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_slot = solana_client.get_slot()?;
    
    loop {
        // 获取最新 slot
        let current_slot = solana_client.get_slot()?;
        
        if current_slot > last_slot {
            println!("📦 New Solana block at slot {}", current_slot);
            
            // 获取区块信息
            let block = solana_client.get_block(current_slot)?;
            
            // 生成 SP1 证明
            let proof = generate_solana_proof(&block).await?;
            
            // 提交到 Ethereum
            submit_to_ethereum(&eth_provider, proof, &block).await?;
            
            last_slot = current_slot;
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
    }
}

/// Ethereum → Solana 中继
async fn relay_eth_to_solana() -> Result<(), Box<dyn std::error::Error>> {
    // 类似实现
    Ok(())
}

/// 生成 Solana 区块的 SP1 证明
async fn generate_solana_proof(
    block: &solana_transaction_status::EncodedConfirmedBlock,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let client = ProverClient::new();
    
    // 准备输入
    let mut stdin = SP1Stdin::new();
    // stdin.write(&prev_blockhash);
    // stdin.write(&block_proof);
    
    // 生成证明
    let (pk, vk) = client.setup(SOLANA_VERIFIER_ELF);
    let proof = client.prove(&pk, stdin).run()?;
    
    // 压缩成 Groth16
    let groth16_proof = client.compress(&vk, proof).groth16().run()?;
    
    Ok(groth16_proof.bytes())
}

async fn submit_to_ethereum(
    provider: &Provider<Http>,
    proof: Vec<u8>,
    block: &solana_transaction_status::EncodedConfirmedBlock,
) -> Result<(), Box<dyn std::error::Error>> {
    // 调用 SolanaUpdater 合约
    println!("✅ Submitting proof to Ethereum...");
    Ok(())
}
```

---

## 开发计划

### Week 1: 环境搭建

#### Day 1-2: 基础环境
```bash
# 安装 Solana
sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"

# 安装 Anchor
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install latest
avm use latest

# 安装 SP1
cargo install sp1-cli
cargo prove --version

# 安装 Hardhat
npm install -g hardhat
```

#### Day 3-4: 项目初始化
```bash
# 创建项目结构
mkdir solana-eth-bridge
cd solana-eth-bridge

# Solana 项目
anchor init bridge --no-git

# SP1 项目
mkdir sp1-programs
cd sp1-programs
cargo prove new solana-verifier
cargo prove new eth-verifier

# Ethereum 项目
mkdir ethereum
cd ethereum
npx hardhat init

# Relayer 项目
cargo new relayer
```

#### Day 5-7: 本地测试网启动
```bash
# 启动 Solana 测试验证器
solana-test-validator --reset \
  --slots-per-epoch 32 \
  --quiet

# 启动 Hardhat 节点
npx hardhat node

# 测试连接
solana cluster-version
cast client --rpc-url http://localhost:8545
```

### Week 2-3: 核心开发

#### Solana → ETH 方向 (Week 2)
- [ ] SP1 Solana 验证程序
- [ ] Ethereum Updater 合约
- [ ] 基础 Relayer (Solana 监听)
- [ ] 端到端测试

#### ETH → Solana 方向 (Week 3)
- [ ] SP1 ETH 验证程序
- [ ] Solana Bridge Program
- [ ] Relayer (ETH 监听)
- [ ] 双向集成测试

### Week 4: Token Bridge

- [ ] Solana Token Vault
- [ ] ETH Token Bridge 合约
- [ ] 跨链转账流程
- [ ] 完整测试

---

## 测试指南

### 1. 单元测试

```bash
# Solana Program
anchor test

# Ethereum Contract
npx hardhat test

# SP1 Programs
cd sp1-programs/solana-verifier
cargo test --release
```

### 2. 集成测试

```bash
# 启动所有服务
./scripts/start-local-env.sh

# 运行集成测试
cargo test --test integration_tests --release
```

### 3. 手动测试流程

```bash
# 1. 部署 Solana Program
anchor deploy

# 2. 部署 ETH Contract
npx hardhat run scripts/deploy.js --network localhost

# 3. 启动 Relayer
cd relayer
cargo run --release

# 4. 发起跨链转账
solana transfer <recipient> 1 SOL --allow-unfunded-recipient

# 5. 观察 Relayer 日志
# 应该看到: 区块捕获 → 证明生成 → ETH 提交
```

---

## 预期性能

| 指标 | 数值 | 备注 |
|------|------|------|
| **Solana 块验证** | ~30-45秒 | SP1 STARK + Groth16 |
| **ETH 块验证** | ~10-20秒 | 更简单的验证逻辑 |
| **ETH Gas 成本** | ~300K gas | Groth16 验证 |
| **Solana 计算单元** | ~200K CU | 简化验证 |
| **延迟 (端到端)** | ~1-2分钟 | 包含确认时间 |

---

## 下一步

1. **现在开始**: 创建项目结构
2. **第一个里程碑**: Solana → ETH 单向桥
3. **完整功能**: 双向 + Token Bridge
4. **优化**: 批量处理、Gas 优化

需要我开始创建项目结构和初始代码吗？
