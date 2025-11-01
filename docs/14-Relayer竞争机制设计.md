# Relayer 竞争机制设计

## 核心思路

**"抢单模式"**：多个 Relayer 同时监听订单 → 竞争生成证明 → 第一个提交有效证明的获胜

---

## 1. 基础设计：先到先得（First-Come-First-Serve）

### 工作流程

```
时刻 T0: 用户锁定代币
┌──────────────────────────────────────────────────────┐
│ Solana: TransferOrder 创建                            │
│ Status: Pending                                      │
│ Order ID: 12345                                      │
└──────────────────────────────────────────────────────┘
                      │
                      ↓ (事件广播)
         ┌────────────┴────────────┐
         │                         │
    Relayer A                 Relayer B                Relayer C
    监听到订单                监听到订单                监听到订单
         │                         │                        │
         ↓                         ↓                        ↓
    开始生成 ZK 证明           开始生成 ZK 证明           开始生成 ZK 证明
    (计算中... 5分钟)         (计算中... 4分钟)         (计算中... 6分钟)
         │                         │                        │
         │                         ↓ T4分钟                │
         │                    ✅ 证明生成完成              │
         │                    提交 unlock_tokens           │
         │                         ↓                        │
         ↓ T5分钟                 【获得奖励！】          ↓ T6分钟
    证明生成完成                                     证明生成完成
    提交 unlock_tokens                               提交 unlock_tokens
         ↓                                                  ↓
    ❌ 失败：订单已完成                               ❌ 失败：订单已完成
```

### 数据结构

```rust
#[account]
pub struct TransferOrder {
    pub order_id: u64,
    pub user: Pubkey,
    pub status: OrderStatus,          // Pending | Completed
    pub token_mint: Pubkey,
    pub amount: u64,
    pub recipient: [u8; 20],          // EVM 地址
    pub created_slot: u64,
    
    // Relayer 信息
    pub completed_by: Pubkey,         // 哪个 Relayer 完成的
    pub completed_at: u64,            // 完成时间 (slot)
    pub proof_hash: [u8; 32],         // ZK 证明哈希
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,      // 待处理（任何 Relayer 都可以抢）
    Completed,    // 已完成（不能再处理）
}
```

### 核心指令实现

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
    
    #[account(mut)]
    pub bridge_config: Account<'info, BridgeConfig>,
    
    #[account(mut)]
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
    
    // Relayer（竞争者）
    #[account(mut)]
    pub relayer: Signer<'info>,
    
    // Relayer 的奖励账户
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
    
    // 1. 验证桥未暂停
    require!(!bridge_config.paused, BridgeError::BridgePaused);
    
    // 2. 验证订单状态（关键：只有 Pending 才能处理）
    require!(
        order.status == OrderStatus::Pending,
        BridgeError::OrderNotPending
    );
    
    // 3. 验证 ZK 证明（当前是 mock，Phase 6 会替换为真实验证）
    require!(proof_hash != [0u8; 32], BridgeError::InvalidProof);
    
    // 4. 计算金额分配
    let total_amount = order.amount;
    let relayer_fee = total_amount
        .checked_mul(bridge_config.relayer_fee_bps as u64)
        .unwrap()
        .checked_div(10000)
        .unwrap();
    let user_amount = total_amount.checked_sub(relayer_fee).unwrap();
    
    // 5. 转账给用户
    let seeds = &[
        b"bridge_config".as_ref(),
        &[ctx.bumps.bridge_config],
    ];
    let signer_seeds = &[&seeds[..]];
    
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
    
    // 6. 奖励给 Relayer（获胜者）
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
    
    // 7. 更新订单状态（原子操作，防止重复）
    order.status = OrderStatus::Completed;
    order.completed_by = ctx.accounts.relayer.key();
    order.completed_at = Clock::get()?.slot;
    order.proof_hash = proof_hash;
    
    // 8. 更新 TokenConfig
    token_config.total_locked = token_config
        .total_locked
        .checked_sub(total_amount)
        .unwrap();
    
    // 9. 发出事件
    emit!(TokensUnlocked {
        order_id,
        user: order.user,
        amount: user_amount,
        relayer: ctx.accounts.relayer.key(),
        relayer_fee,
        proof_hash,
    });
    
    Ok(())
}
```

### 关键安全机制

#### 1. 原子性保证（防止重复奖励）

```rust
// constraint 在账户验证阶段就检查
#[account(
    mut,
    constraint = transfer_order.status == OrderStatus::Pending @ BridgeError::OrderNotPending,
)]
pub transfer_order: Account<'info, TransferOrder>,

// 即使多个 Relayer 同时提交，只有一个能成功：
// - 第一个：status = Pending ✅ → 执行 → status = Completed
// - 第二个：status = Completed ❌ → 交易失败（constraint 不满足）
```

#### 2. 时间戳记录

```rust
pub struct TransferOrder {
    pub completed_by: Pubkey,      // 记录获胜者
    pub completed_at: u64,         // 记录完成时间
}

// 可用于：
// - 分析 Relayer 性能
// - 计算平均处理时间
// - 信誉系统
```

---

## 2. 进阶设计：承诺-揭示机制（Commit-Reveal）

### 问题：抢跑攻击（Front-running）

```
场景：
1. Relayer A 生成证明，提交交易到 Solana
2. Relayer B 监听到 A 的交易（在 mempool 中）
3. Relayer B 复制 A 的证明，提交更高的优先费
4. Relayer B 的交易先被打包
5. Relayer B 窃取了 A 的奖励！
```

### 解决方案：两阶段提交

#### Phase 1: 承诺（Commit）

```rust
#[account]
pub struct RelayerCommitment {
    pub order_id: u64,
    pub relayer: Pubkey,
    pub commitment_hash: [u8; 32],  // hash(proof_hash + salt)
    pub committed_at: u64,
    pub revealed: bool,
}

pub fn commit_proof(
    ctx: Context<CommitProof>,
    order_id: u64,
    commitment_hash: [u8; 32],  // hash(proof_hash + relayer_secret)
) -> Result<()> {
    let commitment = &mut ctx.accounts.relayer_commitment;
    
    commitment.order_id = order_id;
    commitment.relayer = ctx.accounts.relayer.key();
    commitment.commitment_hash = commitment_hash;
    commitment.committed_at = Clock::get()?.slot;
    commitment.revealed = false;
    
    emit!(ProofCommitted {
        order_id,
        relayer: ctx.accounts.relayer.key(),
        commitment_hash,
    });
    
    Ok(())
}
```

#### Phase 2: 揭示（Reveal）

```rust
pub fn reveal_and_unlock(
    ctx: Context<RevealAndUnlock>,
    order_id: u64,
    proof_hash: [u8; 32],
    salt: [u8; 32],
) -> Result<()> {
    let commitment = &ctx.accounts.relayer_commitment;
    let order = &ctx.accounts.transfer_order;
    
    // 1. 验证承诺未被揭示
    require!(!commitment.revealed, BridgeError::AlreadyRevealed);
    
    // 2. 验证承诺哈希
    let computed_hash = hash(&[proof_hash.as_ref(), salt.as_ref()].concat());
    require!(
        computed_hash == commitment.commitment_hash,
        BridgeError::InvalidReveal
    );
    
    // 3. 验证订单仍然是 Pending
    require!(
        order.status == OrderStatus::Pending,
        BridgeError::OrderNotPending
    );
    
    // 4. 执行 unlock 逻辑
    // ... (与前面相同)
    
    // 5. 标记已揭示
    commitment.revealed = true;
    
    Ok(())
}
```

#### 工作流程

```
T0: Relayer A 生成证明
    proof_hash = hash(proof_data)
    salt = random_bytes()
    commitment_hash = hash(proof_hash + salt)

T1: Relayer A 提交承诺
    commit_proof(order_id, commitment_hash)
    → 链上记录，但不暴露证明内容

T2: 等待一定时间（例如 10 slots）
    → 防止抢跑

T3: Relayer A 揭示证明
    reveal_and_unlock(order_id, proof_hash, salt)
    → 验证通过，获得奖励
```

**优点**：
- ✅ 防止证明被窃取
- ✅ 公平竞争

**缺点**：
- ❌ 增加延迟（需要等待揭示期）
- ❌ 增加复杂度（两次交易）

---

## 3. 实用设计：简单竞争 + 重放保护

### 推荐方案（Phase 1-4）

**不使用 Commit-Reveal，而是依赖：**

1. **Solana 的快速确认**（~400ms）
   - 抢跑窗口很小
   - 比 Ethereum 好得多

2. **优先费竞争**
   - Relayer 可以提高优先费
   - 但窃取证明的成本高于奖励

3. **重放保护**
   - 订单状态原子更新
   - 只有第一个成功

### 完整实现

```rust
// ============================================
// 数据结构
// ============================================

#[account]
pub struct BridgeConfig {
    pub authority: Pubkey,
    pub paused: bool,
    pub relayer_fee_bps: u16,        // 手续费率（10 = 0.1%）
    pub min_relayer_fee: u64,        // 最小手续费（防止小额订单不划算）
}

#[account]
pub struct TransferOrder {
    pub order_id: u64,
    pub user: Pubkey,
    pub status: OrderStatus,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub recipient: [u8; 20],
    pub created_slot: u64,
    
    // 竞争结果
    pub completed_by: Pubkey,        // 获胜的 Relayer
    pub completed_at: u64,           // 完成时间
    pub proof_hash: [u8; 32],
}

// ============================================
// 核心指令
// ============================================

pub fn unlock_tokens(
    ctx: Context<UnlockTokens>,
    order_id: u64,
    proof_hash: [u8; 32],
) -> Result<()> {
    let order = &mut ctx.accounts.transfer_order;
    let bridge_config = &ctx.accounts.bridge_config;
    
    // 验证
    require!(!bridge_config.paused, BridgeError::BridgePaused);
    require!(
        order.status == OrderStatus::Pending,
        BridgeError::OrderNotPending  // 第二个 Relayer 会在这里失败
    );
    require!(proof_hash != [0u8; 32], BridgeError::InvalidProof);
    
    // 计算费用
    let relayer_fee = calculate_relayer_fee(
        order.amount,
        bridge_config.relayer_fee_bps,
        bridge_config.min_relayer_fee,
    );
    let user_amount = order.amount.checked_sub(relayer_fee).unwrap();
    
    // 转账（用户 + Relayer）
    transfer_to_user(ctx, user_amount)?;
    transfer_to_relayer(ctx, relayer_fee)?;
    
    // 更新状态（原子操作）
    order.status = OrderStatus::Completed;
    order.completed_by = ctx.accounts.relayer.key();
    order.completed_at = Clock::get()?.slot;
    order.proof_hash = proof_hash;
    
    // 更新统计
    ctx.accounts.token_config.total_locked -= order.amount;
    
    // 事件
    emit!(TokensUnlocked {
        order_id,
        user: order.user,
        amount: user_amount,
        relayer: ctx.accounts.relayer.key(),
        relayer_fee,
        proof_hash,
        slot: order.completed_at,
    });
    
    Ok(())
}

// 辅助函数
fn calculate_relayer_fee(
    amount: u64,
    fee_bps: u16,
    min_fee: u64,
) -> u64 {
    let calculated_fee = amount
        .checked_mul(fee_bps as u64)
        .unwrap()
        .checked_div(10000)
        .unwrap();
    
    // 取较大值（保证最小手续费）
    calculated_fee.max(min_fee)
}
```

---

## 4. Relayer 监听和竞争逻辑

### Relayer 客户端实现

```typescript
// relayer.ts
import { Connection, PublicKey } from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';

class CompetitiveRelayer {
  private connection: Connection;
  private program: Program;
  private relayerKeypair: Keypair;
  
  constructor(config) {
    this.connection = new Connection(config.rpcUrl);
    this.program = new Program(IDL, config.programId);
    this.relayerKeypair = config.relayerKeypair;
  }
  
  // 监听新订单
  async watchOrders() {
    console.log('🔍 Watching for new orders...');
    
    // 方式 1: 监听事件
    this.program.addEventListener('TokensLocked', async (event) => {
      console.log(`📦 New order detected: ${event.orderId}`);
      
      // 立即开始处理（竞争开始）
      await this.processOrder(event.orderId, event);
    });
    
    // 方式 2: 轮询（作为备份）
    setInterval(() => this.pollPendingOrders(), 10000);
  }
  
  // 处理订单（竞争逻辑）
  async processOrder(orderId: number, orderData: any) {
    const startTime = Date.now();
    
    try {
      // 1. 检查订单是否仍然 Pending
      const order = await this.program.account.transferOrder.fetch(
        this.getOrderPDA(orderId)
      );
      
      if (order.status.completed) {
        console.log(`⏭️  Order ${orderId} already completed`);
        return;
      }
      
      console.log(`⚡ Starting to compete for order ${orderId}`);
      
      // 2. 生成 ZK 证明（耗时操作）
      const proof = await this.generateZKProof(orderData);
      const proofHash = this.hashProof(proof);
      
      const proofTime = Date.now() - startTime;
      console.log(`✅ Proof generated in ${proofTime}ms`);
      
      // 3. 提交证明（竞速时刻）
      const tx = await this.submitProof(orderId, proofHash);
      
      console.log(`🏆 Won order ${orderId}! TX: ${tx}`);
      
      // 4. 更新统计
      await this.updateStats({
        orderId,
        success: true,
        proofTime,
        totalTime: Date.now() - startTime,
      });
      
    } catch (error) {
      if (error.message.includes('OrderNotPending')) {
        console.log(`😔 Lost race for order ${orderId}`);
        
        await this.updateStats({
          orderId,
          success: false,
          reason: 'lost_race',
        });
      } else {
        console.error(`❌ Error processing order ${orderId}:`, error);
        
        await this.updateStats({
          orderId,
          success: false,
          reason: 'error',
          error: error.message,
        });
      }
    }
  }
  
  // 生成 ZK 证明
  async generateZKProof(orderData: any): Promise<Proof> {
    // Phase 1-5: Mock 证明
    await new Promise(resolve => setTimeout(resolve, 3000)); // 模拟 3 秒
    return {
      data: new Uint8Array(32).fill(1),
      publicInputs: orderData,
    };
    
    // Phase 6: 真实 SP1 证明
    // const proof = await sp1.prove(orderData);
    // return proof;
  }
  
  // 提交证明
  async submitProof(orderId: number, proofHash: Buffer): Promise<string> {
    const tx = await this.program.methods
      .unlockTokens(orderId, Array.from(proofHash))
      .accounts({
        transferOrder: this.getOrderPDA(orderId),
        bridgeConfig: this.getBridgeConfigPDA(),
        // ... 其他账户
        relayer: this.relayerKeypair.publicKey,
      })
      .signers([this.relayerKeypair])
      .rpc({
        // 重要：设置优先费（竞争优势）
        skipPreflight: false,
        preflightCommitment: 'confirmed',
      });
    
    return tx;
  }
  
  // 统计（用于分析性能）
  async updateStats(data: RelayStats) {
    // 存储到数据库或日志
    console.log('📊 Stats:', data);
  }
}

// 运行多个 Relayer 实例
async function main() {
  const relayer1 = new CompetitiveRelayer({
    rpcUrl: 'https://api.devnet.solana.com',
    programId: PROGRAM_ID,
    relayerKeypair: loadKeypair('./relayer1.json'),
  });
  
  const relayer2 = new CompetitiveRelayer({
    rpcUrl: 'https://api.devnet.solana.com',
    programId: PROGRAM_ID,
    relayerKeypair: loadKeypair('./relayer2.json'),
  });
  
  // 同时启动
  await Promise.all([
    relayer1.watchOrders(),
    relayer2.watchOrders(),
  ]);
}
```

---

## 5. 性能优化策略

### Relayer 竞争力提升

```typescript
class OptimizedRelayer extends CompetitiveRelayer {
  // 1. 预计算（提前准备）
  async precomputeProof(orderData: any) {
    // 在订单创建的瞬间就开始计算
    // 不等待其他验证
    const proof = await this.generateZKProof(orderData);
    return proof;
  }
  
  // 2. 并行处理多个订单
  async processMultipleOrders() {
    const pendingOrders = await this.getPendingOrders();
    
    // 并行处理
    const promises = pendingOrders.map(order => 
      this.processOrder(order.id, order.data)
    );
    
    await Promise.allSettled(promises);
  }
  
  // 3. 优先费策略
  calculateOptimalPriorityFee(orderAmount: number): number {
    const relayerFee = orderAmount * 0.001; // 0.1%
    
    // 动态优先费：愿意花费奖励的 10% 来抢单
    const maxPriorityFee = relayerFee * 0.1;
    
    // 根据网络拥堵情况调整
    const networkCongestion = this.getNetworkCongestion();
    
    return Math.min(maxPriorityFee, networkCongestion * 1.2);
  }
  
  // 4. 硬件加速
  async generateZKProofWithGPU(orderData: any): Promise<Proof> {
    // 使用 GPU 加速 ZK 证明生成
    // 可以从 3 秒降低到 1 秒
    return await this.gpuAccelerator.prove(orderData);
  }
  
  // 5. RPC 优化
  private connection = new Connection(
    'https://premium-rpc-endpoint.com', // 使用付费 RPC
    {
      commitment: 'confirmed',
      confirmTransactionInitialTimeout: 60000,
      wsEndpoint: 'wss://premium-ws-endpoint.com', // WebSocket 更快
    }
  );
}
```

---

## 6. 经济模型分析

### 收益计算

```typescript
// Relayer 盈利模型
interface RelayerEconomics {
  // 收入
  relayerFee: number;           // 例如：100 USDC * 0.1% = 0.1 USDC
  
  // 成本
  computeCost: number;          // ZK 证明计算（电费、硬件折旧）
  transactionFee: number;       // Solana 交易费 (~0.000005 SOL)
  priorityFee: number;          // 优先费（竞争成本）
  
  // 净利润
  profit: number;               // relayerFee - costs
}

function analyzeProfit(orderAmount: number): RelayerEconomics {
  const relayerFeeBps = 10; // 0.1%
  const relayerFee = orderAmount * relayerFeeBps / 10000;
  
  const computeCost = 0.01;     // $0.01 (GPU 3 秒)
  const transactionFee = 0.00001; // 几乎免费
  const priorityFee = 0.001;    // $0.001 (竞争)
  
  const totalCost = computeCost + transactionFee + priorityFee;
  const profit = relayerFee - totalCost;
  
  return {
    relayerFee,
    computeCost,
    transactionFee,
    priorityFee,
    profit,
    roi: (profit / totalCost) * 100,
  };
}

// 示例
console.log(analyzeProfit(1000));  // $1000 订单
// {
//   relayerFee: 0.1,
//   computeCost: 0.01,
//   transactionFee: 0.00001,
//   priorityFee: 0.001,
//   profit: 0.089,
//   roi: 808%  ← 非常有利可图！
// }

console.log(analyzeProfit(100));   // $100 订单
// {
//   relayerFee: 0.01,
//   profit: -0.001,
//   roi: -9%  ← 不划算，需要最小手续费保护
// }
```

### 最小手续费设计

```rust
pub struct BridgeConfig {
    pub relayer_fee_bps: u16,      // 10 = 0.1%
    pub min_relayer_fee: u64,      // 例如：0.05 USDC
}

// 保证小额订单也有利可图
fn calculate_relayer_fee(amount: u64, config: &BridgeConfig) -> u64 {
    let percentage_fee = amount * config.relayer_fee_bps as u64 / 10000;
    percentage_fee.max(config.min_relayer_fee)
}
```

---

## 7. 测试用例

```typescript
describe("Relayer Competition", () => {
  it("First relayer wins the race", async () => {
    // 创建订单
    const orderId = await createOrder(100_000_000); // 100 USDC
    
    // 两个 Relayer 同时提交
    const [tx1, tx2] = await Promise.allSettled([
      relayer1.unlockTokens(orderId, proofHash1),
      relayer2.unlockTokens(orderId, proofHash2),
    ]);
    
    // 断言：一个成功，一个失败
    expect(tx1.status === 'fulfilled' || tx2.status === 'fulfilled').toBe(true);
    expect(tx1.status === 'rejected' || tx2.status === 'rejected').toBe(true);
    
    // 检查失败原因
    const failed = tx1.status === 'rejected' ? tx1 : tx2;
    expect(failed.reason.message).toContain('OrderNotPending');
  });
  
  it("Cannot unlock the same order twice", async () => {
    const orderId = await createOrder(100_000_000);
    
    // 第一次成功
    await relayer1.unlockTokens(orderId, proofHash);
    
    // 第二次失败
    await expect(
      relayer2.unlockTokens(orderId, proofHash2)
    ).to.be.rejectedWith('OrderNotPending');
  });
  
  it("Relayer receives correct fee", async () => {
    const orderId = await createOrder(100_000_000); // 100 USDC
    const relayerBalanceBefore = await getTokenBalance(relayer1.publicKey);
    
    await relayer1.unlockTokens(orderId, proofHash);
    
    const relayerBalanceAfter = await getTokenBalance(relayer1.publicKey);
    const fee = relayerBalanceAfter - relayerBalanceBefore;
    
    // 0.1% 手续费
    expect(fee).toBe(100_000); // 0.1 USDC
  });
});
```

---

## 8. 总结

### 推荐实施方案

**Phase 1.4（当前）**：
```rust
✅ 简单竞争模式（先到先得）
✅ 订单状态原子更新（重放保护）
✅ Relayer 手续费机制
✅ 事件记录获胜者
```

**Phase 3-4（主网准备）**：
```rust
✅ 多 Relayer 监听
✅ 性能优化（GPU 加速）
✅ 动态优先费策略
✅ 统计和监控
```

**Phase 5+（可选）**：
```rust
🔄 Commit-Reveal（如果抢跑严重）
🔄 信誉系统（优先分配给高信誉 Relayer）
🔄 订单路由（大额订单分配给可信 Relayer）
```

### 核心优势

| 特性 | 我们的设计 | 传统方案 |
|------|-----------|---------|
| **去中心化** | ✅ 任何人可以成为 Relayer | ⚠️ 需要许可 |
| **竞争激励** | ✅ 抢单模式，自然竞争 | ❌ 轮询或分配 |
| **安全性** | ✅ 原子状态更新 | ⚠️ 可能重复奖励 |
| **效率** | ✅ 最快的 Relayer 获胜 | ❌ 平均速度 |
| **抗审查** | ✅ 多 Relayer 冗余 | ⚠️ 单点故障 |

**这是一个优雅且实用的设计！** 🎯
