# Sprint 5: SP1 ZK 证明核心实现 ⚡

> **优先级**: 🔥 最高优先级 - 这是 zkBridge 的核心功能!
> 
> **前置条件**: Sprint 0-4 完成
> 
> **时间估算**: 5-7 天

---

## 🎯 目标

实现 zkBridge 的核心机制：**ZK 证明的生成和验证**

### 当前问题

已完成的 Sprint 1-4 只实现了:
- ✅ 监控 Solana 和 Ethereum 区块
- ✅ 验证区块连续性逻辑
- ✅ 合约部署和数据结构

但**完全缺失**:
- ❌ SP1 zkVM 证明生成
- ❌ 链上证明验证
- ❌ Groth16 压缩证明

这相当于建了桥墩,但没有桥面!

---

## 📋 任务分解

### Task 1: 安装 SP1 工具链 (1 天)

#### 1.1 安装 SP1 CLI
```bash
cd /workspace
curl -L https://sp1.succinct.xyz | bash
sp1up

# 验证安装
sp1 --version
```

#### 1.2 更新 Relayer 依赖
```toml
# relayer/Cargo.toml
[dependencies]
sp1-sdk = "1.2.0"

[build-dependencies]
sp1-helper = "1.2.0"
```

#### 1.3 创建 build.rs
```rust
// relayer/build.rs
use sp1_helper::{build_program_with_args, BuildArgs};

fn main() {
    // 构建 Solana 验证器程序
    build_program_with_args(
        "../sp1-programs/solana-verifier",
        BuildArgs::default(),
    );
    
    // 构建 Ethereum 验证器程序
    build_program_with_args(
        "../sp1-programs/eth-verifier",
        BuildArgs::default(),
    );
}
```

**验收标准**:
- [ ] `sp1 --version` 输出版本号
- [ ] `cargo build` 成功构建 ELF 文件
- [ ] 在 `target/release/build/` 下生成 ELF

---

### Task 2: 实现 SP1 证明生成器 (2-3 天)

#### 2.1 创建 Prover 模块
```rust
// relayer/src/prover/mod.rs
use sp1_sdk::{ProverClient, SP1Stdin};

pub struct Sp1Prover {
    client: ProverClient,
    solana_elf: &'static [u8],
    eth_elf: &'static [u8],
}

impl Sp1Prover {
    pub fn new() -> Self {
        let client = ProverClient::new();
        
        Self {
            client,
            solana_elf: include_bytes!("../../elf/solana-verifier"),
            eth_elf: include_bytes!("../../elf/eth-verifier"),
        }
    }
    
    /// 为 Solana 区块生成证明
    pub fn prove_solana_block(
        &self,
        block_data: &SolanaBlockData,
    ) -> Result<SP1ProofWithPublicValues, Error> {
        let mut stdin = SP1Stdin::new();
        
        // 写入输入数据
        stdin.write(&block_data.slot);
        stdin.write(&block_data.parent_slot);
        stdin.write(&block_data.blockhash);
        stdin.write(&block_data.signatures);
        
        // 生成 STARK 证明
        println!("🔬 Generating STARK proof for Solana block {}...", block_data.slot);
        let proof = self.client.prove(self.solana_elf, stdin)
            .run()
            .map_err(|e| Error::ProofGeneration(e.to_string()))?;
        
        Ok(proof)
    }
    
    /// 为 Ethereum 区块生成证明
    pub fn prove_ethereum_block(
        &self,
        block_data: &EthereumBlockData,
    ) -> Result<SP1ProofWithPublicValues, Error> {
        let mut stdin = SP1Stdin::new();
        
        stdin.write(&block_data.number);
        stdin.write(&block_data.parent_hash);
        stdin.write(&block_data.hash);
        stdin.write(&block_data.timestamp);
        
        println!("🔬 Generating STARK proof for Ethereum block {}...", block_data.number);
        let proof = self.client.prove(self.eth_elf, stdin)
            .run()
            .map_err(|e| Error::ProofGeneration(e.to_string()))?;
        
        Ok(proof)
    }
    
    /// 压缩为 Groth16 证明 (用于 Ethereum 链上验证)
    pub fn compress_to_groth16(
        &self,
        proof: SP1ProofWithPublicValues,
    ) -> Result<Vec<u8>, Error> {
        println!("🗜️  Compressing proof to Groth16...");
        
        let compressed = self.client.compress(proof)
            .map_err(|e| Error::Compression(e.to_string()))?;
        
        Ok(compressed.bytes())
    }
}
```

#### 2.2 集成到 Solana Monitor
```rust
// relayer/src/solana_monitor.rs
use crate::prover::Sp1Prover;

pub struct SolanaMonitor {
    // ... 现有字段
    prover: Arc<Sp1Prover>,
    proof_queue: Arc<Mutex<VecDeque<SolanaProof>>>,
}

impl SolanaMonitor {
    pub async fn start(&self) -> Result<()> {
        loop {
            if let Some(new_slot) = self.get_confirmed_slot().await? {
                // 1. 获取区块完整数据
                let block = self.get_block_data(new_slot).await?;
                
                // 2. 生成 ZK 证明
                let proof = self.prover.prove_solana_block(&block)?;
                
                // 3. 压缩证明
                let groth16_proof = self.prover.compress_to_groth16(proof)?;
                
                // 4. 提交到 Ethereum
                println!("✅ Generated proof for Solana block {}", new_slot);
                self.submit_to_ethereum(block, groth16_proof).await?;
            }
            
            sleep(Duration::from_millis(400)).await;
        }
    }
}
```

#### 2.3 集成到 Ethereum Monitor
```rust
// relayer/src/ethereum_monitor.rs
impl EthereumMonitor {
    pub async fn start(&self) -> Result<()> {
        loop {
            if let Some(new_block) = self.get_confirmed_block().await? {
                // 1. 获取区块头详细数据
                let block_data = self.get_block_header(new_block).await?;
                
                // 2. 生成 ZK 证明
                let proof = self.prover.prove_ethereum_block(&block_data)?;
                
                // 3. 提交到 Solana (Solana 使用 STARK 证明)
                println!("✅ Generated proof for Ethereum block {}", new_block);
                self.submit_to_solana(block_data, proof).await?;
            }
            
            sleep(Duration::from_secs(12)).await;
        }
    }
}
```

**验收标准**:
- [ ] 能为 Solana 区块生成 STARK 证明
- [ ] 能为 Ethereum 区块生成 STARK 证明
- [ ] 能将 STARK 压缩为 Groth16
- [ ] 证明生成时间 < 30 秒
- [ ] 输出证明大小信息

---

### Task 3: 部署 SP1 Groth16 验证器合约 (1 天)

#### 3.1 获取 SP1 Verifier 合约
```bash
# SP1 官方提供的 Groth16 验证器合约
# https://github.com/succinctlabs/sp1-contracts

cd /workspace/ethereum
forge install succinctlabs/sp1-contracts
```

#### 3.2 更新部署脚本
```javascript
// ethereum/scripts/deploy.js
const { ethers } = require("hardhat");

async function main() {
    // 1. 部署 SP1 Groth16 Verifier
    const SP1Verifier = await ethers.getContractFactory("SP1Verifier");
    const sp1Verifier = await SP1Verifier.deploy();
    await sp1Verifier.deployed();
    console.log("✅ SP1Verifier deployed to:", sp1Verifier.address);
    
    // 2. 部署 SolanaUpdater (使用真实 verifier)
    const SolanaUpdater = await ethers.getContractFactory("SolanaUpdater");
    const updater = await SolanaUpdater.deploy(
        sp1Verifier.address,  // 真实的 SP1 Verifier 地址
        0,                    // 初始 slot
        "0x00..."             // 初始 blockhash
    );
    await updater.deployed();
    console.log("✅ SolanaUpdater deployed to:", updater.address);
    
    // 3. 保存地址
    fs.writeFileSync(
        "deployments.json",
        JSON.stringify({
            sp1Verifier: sp1Verifier.address,
            solanaUpdater: updater.address,
        }, null, 2)
    );
}
```

#### 3.3 更新 SolanaUpdater 合约
```solidity
// ethereum/contracts/SolanaUpdater.sol
contract SolanaUpdater {
    ISP1Verifier public immutable verifier;
    bytes32 public immutable programVKey;  // Solana verifier 的 VKey
    
    constructor(address _verifier, bytes32 _vkey) {
        verifier = ISP1Verifier(_verifier);
        programVKey = _vkey;
    }
    
    function updateSolanaState(
        uint64 slot,
        bytes32 blockhash,
        bytes calldata proof,
        bytes calldata publicValues
    ) external {
        // 1. 验证 Groth16 证明
        verifier.verifyProof(programVKey, publicValues, proof);
        
        // 2. 解析 public values
        (uint64 verifiedSlot, bytes32 verifiedHash) = abi.decode(
            publicValues,
            (uint64, bytes32)
        );
        
        // 3. 检查一致性
        require(verifiedSlot == slot, "Slot mismatch");
        require(verifiedHash == blockhash, "Hash mismatch");
        
        // 4. 更新状态
        latestSlot = slot;
        slotToBlockhash[slot] = blockhash;
        
        emit SolanaStateUpdated(slot, blockhash);
    }
}
```

**验收标准**:
- [ ] SP1Verifier 成功部署
- [ ] SolanaUpdater 使用真实 verifier 地址
- [ ] 能成功验证真实的 Groth16 证明
- [ ] 测试用例更新并通过

---

### Task 4: 实现 Solana 链上证明验证 (1-2 天)

#### 4.1 添加 SP1 验证 CPI
```rust
// solana-bridge/programs/solana-bridge/src/lib.rs

// 导入 SP1 Solana 验证器
declare_id!("SP1Verifier111111111111111111111111111111111");

pub mod solana_bridge {
    use super::*;
    
    pub fn verify_eth_block(
        ctx: Context<VerifyEthBlock>,
        block_number: u64,
        block_hash: [u8; 32],
        parent_hash: [u8; 32],
        proof: Vec<u8>,
    ) -> Result<()> {
        msg!("🔍 Verifying Ethereum block {}", block_number);
        
        // 1. 准备 public inputs
        let public_inputs = PublicInputs {
            block_number,
            block_hash,
            parent_hash,
        };
        
        // 2. 调用 SP1 验证器 (CPI)
        let cpi_ctx = CpiContext::new(
            ctx.accounts.sp1_verifier.to_account_info(),
            sp1_verifier::cpi::accounts::Verify {
                proof_account: ctx.accounts.proof_account.to_account_info(),
            },
        );
        
        sp1_verifier::cpi::verify_proof(
            cpi_ctx,
            ctx.accounts.bridge_state.eth_verifier_vkey,
            public_inputs.to_bytes(),
            proof,
        )?;
        
        msg!("✅ Proof verified successfully!");
        
        // 3. 更新状态
        let bridge_state = &mut ctx.accounts.bridge_state;
        
        require!(
            block_number == bridge_state.latest_eth_block + 1,
            ErrorCode::InvalidBlockNumber
        );
        
        bridge_state.latest_eth_block = block_number;
        bridge_state.eth_headers.push_back(EthHeader {
            number: block_number,
            hash: block_hash,
            parent_hash,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        emit!(EthBlockVerified {
            block_number,
            block_hash,
        });
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct VerifyEthBlock<'info> {
    #[account(mut)]
    pub bridge_state: Account<'info, BridgeState>,
    
    /// SP1 验证器程序
    pub sp1_verifier: Program<'info, Sp1Verifier>,
    
    /// 证明账户
    pub proof_account: AccountInfo<'info>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
}
```

#### 4.2 更新 BridgeState
```rust
#[account]
pub struct BridgeState {
    pub latest_eth_block: u64,
    pub eth_headers: VecDeque<EthHeader>,
    
    // SP1 程序验证密钥
    pub eth_verifier_vkey: [u8; 32],  // Ethereum verifier 的 VKey
    pub authority: Pubkey,
}
```

**验收标准**:
- [ ] 能通过 CPI 调用 SP1 验证器
- [ ] 拒绝无效证明
- [ ] 接受有效证明并更新状态
- [ ] 所有测试用例更新并通过

---

### Task 5: 端到端测试 (1 天)

#### 5.1 完整流程测试
```rust
// relayer/tests/integration_test.rs

#[tokio::test]
async fn test_full_solana_to_ethereum_bridge() {
    // 1. 启动本地 Solana 节点
    let solana = start_local_solana().await;
    
    // 2. 启动本地 Ethereum 节点
    let ethereum = start_local_ethereum().await;
    
    // 3. 部署合约
    deploy_contracts(&ethereum).await;
    
    // 4. 启动 Relayer
    let relayer = Relayer::new(solana.clone(), ethereum.clone()).await;
    tokio::spawn(async move { relayer.start().await });
    
    // 5. 在 Solana 上产生新区块
    solana.produce_blocks(5).await;
    
    // 6. 等待 Relayer 处理
    sleep(Duration::from_secs(30)).await;
    
    // 7. 验证 Ethereum 上的状态
    let updater = SolanaUpdater::new(ethereum.clone());
    let latest_slot = updater.latest_slot().await?;
    
    assert!(latest_slot >= 5, "Solana state not updated on Ethereum");
    println!("✅ Solana block {} bridged to Ethereum with ZK proof!", latest_slot);
}

#[tokio::test]
async fn test_full_ethereum_to_solana_bridge() {
    // 类似的测试,但方向相反
    // ...
}

#[tokio::test]
async fn test_proof_generation_performance() {
    let prover = Sp1Prover::new();
    
    let start = Instant::now();
    let proof = prover.prove_solana_block(&mock_block_data()).await?;
    let prove_time = start.elapsed();
    
    println!("⏱️  STARK proof generation: {:?}", prove_time);
    assert!(prove_time < Duration::from_secs(30), "Proof too slow");
    
    let start = Instant::now();
    let groth16 = prover.compress_to_groth16(proof)?;
    let compress_time = start.elapsed();
    
    println!("⏱️  Groth16 compression: {:?}", compress_time);
    println!("📦 Proof size: {} bytes", groth16.len());
    
    assert!(groth16.len() < 1024, "Proof too large");
}
```

**验收标准**:
- [ ] Solana → Ethereum 完整流程通过
- [ ] Ethereum → Solana 完整流程通过
- [ ] 证明生成时间合理 (< 30秒)
- [ ] 证明大小合理 (< 1KB)
- [ ] 无内存泄漏

---

## 🎯 验收标准总结

### 功能性
- [ ] 能为任意 Solana 区块生成有效的 ZK 证明
- [ ] 能为任意 Ethereum 区块生成有效的 ZK 证明
- [ ] Ethereum 合约能验证 Solana 的 Groth16 证明
- [ ] Solana 程序能验证 Ethereum 的 STARK 证明
- [ ] Relayer 自动生成并提交证明

### 性能
- [ ] STARK 证明生成 < 30 秒
- [ ] Groth16 压缩 < 10 秒
- [ ] Groth16 证明大小 < 1 KB
- [ ] 链上验证 gas < 300,000

### 可靠性
- [ ] 证明生成失败能重试
- [ ] 链上验证失败能回滚
- [ ] 所有错误都有日志
- [ ] 集成测试 100% 通过

---

## 🚨 常见问题

### Q1: SP1 证明生成太慢怎么办?
**A**: 使用 `prover.prove_compressed()` 直接生成压缩证明,跳过 STARK 中间步骤

### Q2: Groth16 证明在 Ethereum 上验证失败?
**A**: 检查:
1. VKey 是否匹配
2. Public values 格式是否正确
3. 是否使用了正确的 SP1 版本

### Q3: Solana 上的 CPI 调用失败?
**A**: 确保:
1. SP1 验证器程序已部署
2. Program ID 正确
3. 账户权限设置正确

### Q4: 内存不足?
**A**: SP1 证明生成需要大量内存 (16GB+),可以:
1. 增加 swap
2. 使用云服务器
3. 启用 SP1 的流式证明模式

---

## 📝 完成后的提交

### Git Commit Message
```
feat: Sprint 5 完成 - SP1 ZK 证明核心实现 ⚡

实现了 zkBridge 的核心机制:

✅ SP1 工具链集成
✅ 证明生成器实现 (STARK + Groth16)
✅ Ethereum 链上验证 (SP1Verifier + SolanaUpdater)
✅ Solana 链上验证 (CPI 调用 SP1 验证器)
✅ 端到端集成测试

性能指标:
- STARK 生成: ~20s
- Groth16 压缩: ~8s
- 证明大小: ~800 bytes
- Gas 成本: ~250,000

现在这才是真正的 zkBridge! 🌉
```

### 更新文档
- [ ] 更新 `docs/7-实现进度.md`
- [ ] 添加证明生成流程图
- [ ] 记录性能基准
- [ ] 更新 README

---

## 🎉 Sprint 5 成功标志

当你看到以下输出时,Sprint 5 就完成了:

```
🔬 Generating STARK proof for Solana block 12345...
✅ Proof generated in 19.2s
🗜️  Compressing to Groth16...
✅ Compressed in 7.8s (size: 768 bytes)
📡 Submitting to Ethereum...
✅ Proof verified on-chain! Tx: 0xabcd...
🌉 zkBridge is LIVE!
```

这时你才真正拥有了一个 **零知识跨链桥**! 🚀
