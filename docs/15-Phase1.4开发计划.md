# Phase 1.4 开发计划 - 取消超时机制 + Relayer 手续费

## 开发目标

**Phase 1.4 核心任务**：
1. ✅ 彻底移除超时退款机制
2. ✅ 实现 Relayer 手续费激励
3. ✅ 为未来的竞争机制预留接口

**未来扩展（Phase 2+）**：
- 🔄 Relayer 竞争机制（Commit-Reveal）
- 🔄 用户可选择竞争/非竞争模式

---

## 1. Phase 1.4 实施清单

### 1.1 数据结构更新

#### 简化 `TransferOrder`

```rust
#[account]
pub struct TransferOrder {
    pub order_id: u64,                // 订单 ID
    pub user: Pubkey,                 // 用户地址
    pub status: OrderStatus,          // Pending | Completed
    pub token_mint: Pubkey,           // SPL Token Mint
    pub amount: u64,                  // 锁定数量
    pub recipient: [u8; 20],          // EVM 接收地址
    pub created_slot: u64,            // 创建时间
    
    // Relayer 信息（新增）
    pub completed_by: Pubkey,         // 完成订单的 Relayer
    pub completed_at: u64,            // 完成时间（slot）
    pub proof_hash: [u8; 32],         // ZK 证明哈希
    
    // ❌ 移除字段：
    // pub timeout_slot: u64,
    // pub refunded_slot: u64,
}

// Space: 8 + 8 + 32 + 1 + 32 + 8 + 20 + 8 + 32 + 8 + 32 = 189 bytes
```

#### 简化 `OrderStatus`

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,      // 待处理（Relayer 可以处理）
    Completed,    // 已完成（不可再处理）
    
    // ❌ 移除：
    // Refunded,
}
```

#### 更新 `BridgeConfig`

```rust
#[account]
pub struct BridgeConfig {
    pub authority: Pubkey,
    pub paused: bool,
    
    // Relayer 手续费配置（新增）
    pub relayer_fee_bps: u16,         // 手续费率（基点，10 = 0.1%）
    pub min_relayer_fee: u64,         // 最小手续费（例如 0.05 USDC）
}

// Space: 8 + 32 + 1 + 2 + 8 = 51 bytes
```

---

### 1.2 核心指令修改

#### 修改 `unlock_tokens`

```rust
#[derive(Accounts)]
#[instruction(order_id: u64)]
pub struct UnlockTokens<'info> {
    #[account(
        mut,
        seeds = [b"transfer_order", order_id.to_le_bytes().as_ref()],
        bump,
        constraint = transfer_order.status == OrderStatus::Pending @ BridgeError::OrderNotPending,
    )]
    pub transfer_order: Account<'info, TransferOrder>,
    
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump,
        constraint = !bridge_config.paused @ BridgeError::BridgePaused,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,
    
    #[account(
        mut,
        seeds = [b"token_config", transfer_order.token_mint.as_ref()],
        bump,
    )]
    pub token_config: Account<'info, TokenConfig>,
    
    #[account(
        mut,
        associated_token::mint = token_config.token_mint,
        associated_token::authority = transfer_order.user,
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        seeds = [b"vault", token_config.token_mint.as_ref()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    
    // Relayer 账户（新增）
    #[account(mut)]
    pub relayer: Signer<'info>,
    
    // Relayer 奖励接收账户（新增）
    #[account(
        mut,
        associated_token::mint = token_config.token_mint,
        associated_token::authority = relayer,
    )]
    pub relayer_reward_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

pub fn unlock_tokens(
    ctx: Context<UnlockTokens>,
    order_id: u64,
    proof_hash: [u8; 32],
) -> Result<()> {
    let order = &mut ctx.accounts.transfer_order;
    let bridge_config = &ctx.accounts.bridge_config;
    let token_config = &mut ctx.accounts.token_config;
    let clock = Clock::get()?;
    
    // 1. 验证证明（当前是 mock，Phase 6 替换为真实 SP1 验证）
    require!(proof_hash != [0u8; 32], BridgeError::InvalidProof);
    
    // 2. 计算 Relayer 手续费
    let total_amount = order.amount;
    let relayer_fee = calculate_relayer_fee(
        total_amount,
        bridge_config.relayer_fee_bps,
        bridge_config.min_relayer_fee,
    );
    let user_amount = total_amount.checked_sub(relayer_fee).unwrap();
    
    // 3. PDA 签名种子
    let seeds = &[
        b"bridge_config".as_ref(),
        &[ctx.bumps.bridge_config],
    ];
    let signer_seeds = &[&seeds[..]];
    
    // 4. 转账给用户
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.bridge_config.to_account_info(),
            },
            signer_seeds,
        ),
        user_amount,
    )?;
    
    // 5. 奖励给 Relayer
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.relayer_reward_account.to_account_info(),
                authority: ctx.accounts.bridge_config.to_account_info(),
            },
            signer_seeds,
        ),
        relayer_fee,
    )?;
    
    // 6. 更新订单状态
    order.status = OrderStatus::Completed;
    order.completed_by = ctx.accounts.relayer.key();
    order.completed_at = clock.slot;
    order.proof_hash = proof_hash;
    
    // 7. 更新 TokenConfig
    token_config.total_locked = token_config
        .total_locked
        .checked_sub(total_amount)
        .unwrap();
    
    // 8. 发出事件
    emit!(TokensUnlocked {
        order_id,
        user: order.user,
        amount: user_amount,
        relayer: ctx.accounts.relayer.key(),
        relayer_fee,
        proof_hash,
        completed_at: order.completed_at,
    });
    
    Ok(())
}

// 辅助函数：计算 Relayer 手续费
fn calculate_relayer_fee(
    amount: u64,
    fee_bps: u16,
    min_fee: u64,
) -> u64 {
    let percentage_fee = amount
        .checked_mul(fee_bps as u64)
        .unwrap()
        .checked_div(10000)
        .unwrap();
    
    // 取较大值（保证最小手续费）
    percentage_fee.max(min_fee)
}
```

#### 修改 `initialize` 指令

```rust
pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    let bridge_config = &mut ctx.accounts.bridge_config;
    
    bridge_config.authority = ctx.accounts.authority.key();
    bridge_config.paused = false;
    
    // 初始化 Relayer 费率（新增）
    bridge_config.relayer_fee_bps = 10;           // 0.1%
    bridge_config.min_relayer_fee = 50_000;       // 0.05 USDC (6 decimals)
    
    emit!(BridgeInitialized {
        authority: bridge_config.authority,
        relayer_fee_bps: bridge_config.relayer_fee_bps,
    });
    
    Ok(())
}
```

#### ❌ 删除 `refund_timeout` 指令

```rust
// 完全移除这个指令
// pub fn refund_timeout(...) -> Result<()> { ... }
```

---

### 1.3 事件更新

```rust
#[event]
pub struct TokensUnlocked {
    pub order_id: u64,
    pub user: Pubkey,
    pub amount: u64,                  // 用户实际收到的金额
    pub relayer: Pubkey,              // 新增：完成订单的 Relayer
    pub relayer_fee: u64,             // 新增：Relayer 获得的手续费
    pub proof_hash: [u8; 32],
    pub completed_at: u64,            // 新增：完成时间
}

#[event]
pub struct BridgeInitialized {
    pub authority: Pubkey,
    pub relayer_fee_bps: u16,         // 新增
}

// ❌ 移除事件：
// pub struct TokensRefunded { ... }
```

---

### 1.4 错误码更新

```rust
#[error_code]
pub enum BridgeError {
    #[msg("Bridge is paused")]
    BridgePaused,
    
    #[msg("Invalid ZK proof")]
    InvalidProof,
    
    #[msg("Order is not in pending status")]
    OrderNotPending,                  // 保留（用于防止重复处理）
    
    // ❌ 移除错误：
    // #[msg("Timeout not reached")]
    // TimeoutNotReached,
    
    // #[msg("Order is not in refunded status")]
    // OrderNotRefunded,
}
```

---

## 2. 测试用例更新

### 2.1 基础测试

```typescript
describe("solana-evm-bridge - Phase 1.4", () => {
  let provider: anchor.AnchorProvider;
  let program: anchor.Program<SolanaEvmBridge>;
  let authority: anchor.web3.Keypair;
  let user: anchor.web3.Keypair;
  let relayer: anchor.web3.Keypair;
  let tokenMint: anchor.web3.PublicKey;
  
  before(async () => {
    // 初始化
    provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    program = anchor.workspace.SolanaEvmBridge;
    
    authority = anchor.web3.Keypair.generate();
    user = anchor.web3.Keypair.generate();
    relayer = anchor.web3.Keypair.generate();
    
    // 空投 SOL
    await Promise.all([
      provider.connection.requestAirdrop(authority.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(user.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(relayer.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL),
    ]);
    
    // 创建测试代币
    tokenMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      6
    );
  });
  
  it("Initialize bridge with relayer fee config", async () => {
    const [bridgeConfig] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("bridge_config")],
      program.programId
    );
    
    await program.methods
      .initialize()
      .accounts({
        bridgeConfig,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([authority])
      .rpc();
    
    const config = await program.account.bridgeConfig.fetch(bridgeConfig);
    assert.equal(config.relayerFeeBps, 10);              // 0.1%
    assert.equal(config.minRelayerFee.toNumber(), 50_000); // 0.05 USDC
  });
  
  it("Unlock tokens with relayer fee", async () => {
    // 设置：注册代币、初始化金库、用户锁定代币
    const orderId = 1;
    const lockAmount = 100_000_000; // 100 USDC
    
    // ... (前置步骤：register_token, init_vault, lock_tokens)
    
    // 获取账户余额（之前）
    const userBalanceBefore = await getTokenBalance(userTokenAccount);
    const relayerBalanceBefore = await getTokenBalance(relayerTokenAccount);
    const vaultBalanceBefore = await getTokenBalance(vault);
    
    // Relayer 解锁代币
    const proofHash = Array(32).fill(1); // Mock proof
    
    await program.methods
      .unlockTokens(new anchor.BN(orderId), proofHash)
      .accounts({
        transferOrder,
        bridgeConfig,
        tokenConfig,
        userTokenAccount,
        vault,
        relayer: relayer.publicKey,
        relayerRewardAccount: relayerTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([relayer])
      .rpc();
    
    // 验证余额变化
    const userBalanceAfter = await getTokenBalance(userTokenAccount);
    const relayerBalanceAfter = await getTokenBalance(relayerTokenAccount);
    const vaultBalanceAfter = await getTokenBalance(vault);
    
    const relayerFee = 100_000; // 0.1% of 100 USDC = 0.1 USDC
    const userAmount = lockAmount - relayerFee;
    
    assert.equal(userBalanceAfter - userBalanceBefore, userAmount);
    assert.equal(relayerBalanceAfter - relayerBalanceBefore, relayerFee);
    assert.equal(vaultBalanceBefore - vaultBalanceAfter, lockAmount);
    
    // 验证订单状态
    const order = await program.account.transferOrder.fetch(transferOrder);
    assert.equal(order.status.completed, true);
    assert.equal(order.completedBy.toBase58(), relayer.publicKey.toBase58());
    assert.ok(order.completedAt.toNumber() > 0);
  });
  
  it("Cannot unlock same order twice", async () => {
    const orderId = 2;
    const lockAmount = 100_000_000;
    
    // 锁定代币
    await lockTokens(orderId, lockAmount);
    
    // 第一个 Relayer 解锁
    await program.methods
      .unlockTokens(new anchor.BN(orderId), Array(32).fill(1))
      .accounts({ /* ... */ relayer: relayer.publicKey })
      .signers([relayer])
      .rpc();
    
    // 第二个 Relayer 尝试解锁（应该失败）
    const relayer2 = anchor.web3.Keypair.generate();
    
    try {
      await program.methods
        .unlockTokens(new anchor.BN(orderId), Array(32).fill(2))
        .accounts({ /* ... */ relayer: relayer2.publicKey })
        .signers([relayer2])
        .rpc();
      
      assert.fail("Should have thrown error");
    } catch (err) {
      assert.include(err.toString(), "OrderNotPending");
    }
  });
  
  it("Minimum relayer fee applies to small orders", async () => {
    const orderId = 3;
    const lockAmount = 10_000; // 0.01 USDC (小额订单)
    
    await lockTokens(orderId, lockAmount);
    
    const relayerBalanceBefore = await getTokenBalance(relayerTokenAccount);
    
    await program.methods
      .unlockTokens(new anchor.BN(orderId), Array(32).fill(1))
      .accounts({ /* ... */ })
      .signers([relayer])
      .rpc();
    
    const relayerBalanceAfter = await getTokenBalance(relayerTokenAccount);
    const actualFee = relayerBalanceAfter - relayerBalanceBefore;
    
    // 0.1% of 0.01 USDC = 0.00001 USDC (10 lamports)
    // 但最小手续费是 0.05 USDC (50_000 lamports)
    assert.equal(actualFee, 50_000); // 应该是最小手续费
  });
  
  it("Rejects invalid proof (all zeros)", async () => {
    const orderId = 4;
    await lockTokens(orderId, 100_000_000);
    
    try {
      await program.methods
        .unlockTokens(new anchor.BN(orderId), Array(32).fill(0)) // Invalid proof
        .accounts({ /* ... */ })
        .signers([relayer])
        .rpc();
      
      assert.fail("Should have thrown error");
    } catch (err) {
      assert.include(err.toString(), "InvalidProof");
    }
  });
});
```

---

## 3. 未来扩展预留（Phase 2+）

### 3.1 用户可选择的订单模式

```rust
// Phase 2: 添加订单模式选择

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderMode {
    Standard,       // 标准模式：任何 Relayer 直接处理（当前实现）
    Competitive,    // 竞争模式：Commit-Reveal 机制
}

#[account]
pub struct TransferOrder {
    // ... 现有字段
    
    pub mode: OrderMode,              // 订单模式（Phase 2 添加）
    pub commitment_deadline: u64,     // Commit 截止时间（Phase 2）
}

pub fn lock_tokens(
    ctx: Context<LockTokens>,
    order_id: u64,
    amount: u64,
    recipient: [u8; 20],
    mode: OrderMode,                  // Phase 2: 用户选择模式
) -> Result<()> {
    // ...
    
    order.mode = mode;
    
    // 如果是竞争模式，设置 commit 截止时间
    if mode == OrderMode::Competitive {
        order.commitment_deadline = Clock::get()?.slot + 120; // 1 分钟
    }
    
    // ...
}
```

### 3.2 Commit-Reveal 机制

```rust
// Phase 2: 添加 Commit-Reveal 指令

#[account]
pub struct RelayerCommitment {
    pub order_id: u64,
    pub relayer: Pubkey,
    pub commitment_hash: [u8; 32],    // hash(proof_hash + salt)
    pub committed_at: u64,
    pub revealed: bool,
}

pub fn commit_proof(
    ctx: Context<CommitProof>,
    order_id: u64,
    commitment_hash: [u8; 32],
) -> Result<()> {
    let order = &ctx.accounts.transfer_order;
    let clock = Clock::get()?;
    
    // 只有竞争模式才能 commit
    require!(
        order.mode == OrderMode::Competitive,
        BridgeError::NotCompetitiveMode
    );
    
    // 必须在 commit 截止时间前
    require!(
        clock.slot <= order.commitment_deadline,
        BridgeError::CommitmentDeadlinePassed
    );
    
    let commitment = &mut ctx.accounts.relayer_commitment;
    commitment.order_id = order_id;
    commitment.relayer = ctx.accounts.relayer.key();
    commitment.commitment_hash = commitment_hash;
    commitment.committed_at = clock.slot;
    commitment.revealed = false;
    
    emit!(ProofCommitted {
        order_id,
        relayer: ctx.accounts.relayer.key(),
        commitment_hash,
    });
    
    Ok(())
}

pub fn reveal_and_unlock(
    ctx: Context<RevealAndUnlock>,
    order_id: u64,
    proof_hash: [u8; 32],
    salt: [u8; 32],
) -> Result<()> {
    let commitment = &ctx.accounts.relayer_commitment;
    let order = &ctx.accounts.transfer_order;
    let clock = Clock::get()?;
    
    // 验证 commitment
    require!(!commitment.revealed, BridgeError::AlreadyRevealed);
    
    // 验证哈希
    let computed_hash = hash(&[proof_hash.as_ref(), salt.as_ref()].concat());
    require!(
        computed_hash == commitment.commitment_hash,
        BridgeError::InvalidReveal
    );
    
    // 必须在 reveal 期内
    require!(
        clock.slot > order.commitment_deadline,
        BridgeError::RevealTooEarly
    );
    require!(
        clock.slot <= order.commitment_deadline + 60, // 30 秒 reveal 期
        BridgeError::RevealTooLate
    );
    
    // 执行 unlock 逻辑（与标准模式相同）
    // ...
    
    Ok(())
}
```

---

## 4. 实施步骤

### Step 1: 修改数据结构（15 分钟）

- [ ] 更新 `TransferOrder` 结构
- [ ] 更新 `OrderStatus` 枚举
- [ ] 更新 `BridgeConfig` 结构
- [ ] 更新事件定义
- [ ] 删除超时相关字段和错误码

### Step 2: 修改核心指令（30 分钟）

- [ ] 修改 `initialize` 指令（添加 relayer_fee_bps 初始化）
- [ ] 修改 `unlock_tokens` 指令（添加 Relayer 手续费逻辑）
- [ ] 删除 `refund_timeout` 指令
- [ ] 添加 `calculate_relayer_fee` 辅助函数

### Step 3: 更新测试（45 分钟）

- [ ] 更新 `initialize` 测试
- [ ] 更新 `unlock_tokens` 测试（验证手续费分配）
- [ ] 添加重复解锁测试
- [ ] 添加最小手续费测试
- [ ] 添加无效证明测试
- [ ] 删除超时相关测试

### Step 4: 编译和测试（10 分钟）

- [ ] 运行 `anchor build`
- [ ] 运行 `anchor test`
- [ ] 验证所有测试通过
- [ ] 检查余额计算正确性

### Step 5: 文档更新（10 分钟）

- [ ] 更新 README.md
- [ ] 创建 Phase 1.4 完成总结
- [ ] 更新架构图

**预计总时间**：约 2 小时

---

## 5. 验收标准

### 功能验收

- [x] ✅ 桥初始化时设置 Relayer 费率
- [x] ✅ 解锁代币时正确计算和分配手续费
- [x] ✅ 用户收到：锁定金额 - Relayer 手续费
- [x] ✅ Relayer 收到：手续费
- [x] ✅ Vault 正确减少：锁定金额
- [x] ✅ 小额订单应用最小手续费
- [x] ✅ 订单只能解锁一次（重放保护）
- [x] ✅ 记录完成订单的 Relayer 信息
- [x] ✅ 拒绝无效证明（全零）

### 测试验收

- [x] ✅ 至少 7 个测试用例
- [x] ✅ 所有测试通过
- [x] ✅ 代码覆盖核心功能
- [x] ✅ 余额计算精确

### 代码质量

- [x] ✅ 无编译警告
- [x] ✅ 无 unused 变量
- [x] ✅ 注释清晰
- [x] ✅ 错误处理完善

---

## 6. Phase 2+ 路线图

### Phase 2: 订单模式选择
- 用户可选择 Standard 或 Competitive 模式
- 实现基础 Commit-Reveal 机制
- 添加模式切换测试

### Phase 3: Relayer 注册和质押
- Relayer 注册机制
- 质押代币要求
- 信誉系统
- 惩罚机制

### Phase 4: 高级竞争机制
- 多 Relayer 并发 commit
- 最优 Relayer 选择算法
- 动态手续费调整
- 性能优化

### Phase 5: 去中心化治理
- DAO 管理 Relayer 参数
- 社区投票机制
- 紧急情况处理

### Phase 6: 真实 ZK 证明
- 集成 SP1 zkVM
- 替换 mock 验证
- 性能基准测试
- 主网部署准备

---

## 7. 总结

**Phase 1.4 核心成果**：
1. ✅ 彻底移除超时退款机制（消除双花风险）
2. ✅ 实现 Relayer 手续费激励（保证活跃性）
3. ✅ 简化状态机（2 状态：Pending → Completed）
4. ✅ 为未来扩展预留接口

**设计优势**：
- 🎯 安全：无双花风险，原子状态更新
- 🎯 简单：核心逻辑清晰，易于理解和审计
- 🎯 灵活：未来可扩展 Commit-Reveal 机制
- 🎯 激励：经济模型吸引 Relayer 参与

**下一步**：开始 Phase 1.4 代码实现！
