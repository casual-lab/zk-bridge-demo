# 为什么 zkBridge 中 relayer 必须收集多个块

## 问题背景

在 zkBridge 的设计中，relayer 需要不断收集和中继区块头（block headers），而不是只收集包含特定跨链交易前后的块。这看似增加了不必要的工作量，但实际上这是**区块链共识协议的本质要求**。

## 核心原因

### 1. 共识协议需要验证签名链（Signature Chain）

**关键点**：区块链不是孤立的块，而是一条由密码学签名连接的链。

#### PoS/BFT 共识机制下的验证要求

以 Tendermint、Cosmos 等 BFT 共识为例：

```
区块 r-1              区块 r               区块 r+1
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│ Header r-1  │      │ Header r    │      │ Header r+1  │
│ ───────────│      │ ───────────│      │ ───────────│
│ Parent Hash │◄─────│ Parent Hash │◄─────│ Parent Hash │
│ Validator   │      │ Validator   │      │ Validator   │
│ Signatures  │      │ Signatures  │      │ Signatures  │
│ (2/3+ 投票) │      │ (2/3+ 投票) │      │ (2/3+ 投票) │
└─────────────┘      └─────────────┘      └─────────────┘
      ▲                     ▲                     ▲
      │                     │                     │
    验证器集合          验证器集合            验证器集合
    V₁,V₂,...Vₙ         可能轮换！           V₁',V₂',...Vₙ'
```

**Light Client 验证逻辑**：
```python
def LightCC(LCS_r-1, blkH_r-1, blkH_r) -> bool:
    """
    LCS_r-1: Light Client State，包含当前验证器集合
    blkH_r-1: 前一个区块头
    blkH_r: 新区块头
    """
    # 1. 检查区块连续性
    if blkH_r.parent_hash != hash(blkH_r-1):
        return False
    
    # 2. 检查验证器签名（关键！）
    validator_set = LCS_r-1.validators
    signatures = blkH_r.validator_signatures
    
    # 需要 2/3+ 验证器签名
    if not verify_quorum(signatures, validator_set, threshold=2/3):
        return False
    
    # 3. 检查验证器集合是否轮换
    if blkH_r.contains_validator_change():
        # 必须基于当前验证器集合验证这个变更！
        if not verify_validator_rotation(LCS_r-1, blkH_r):
            return False
    
    return True
```

### 2. 验证器集合的动态变化

**问题**：如果只验证交易所在的块 r，你怎么知道**哪些验证器有权签名这个块**？

#### 示例场景：

```
块 100: 验证器集合 V = {A, B, C, D, E}
块 150: 验证器集合发生轮换 V' = {A, B, C, F, G}
块 200: 你的跨链交易在这里
块 250: 当前最新块
```

**如果只看块 200**：
- ❌ 你不知道当时的验证器集合是谁
- ❌ 你无法验证签名的有效性
- ❌ 攻击者可以伪造一个"块 200"，用错误的验证器签名

**必须从可信起点（块 100）同步到当前**：
- ✅ 块 100 → 101: 用 V 验证
- ✅ 块 149 → 150: 检测到验证器轮换，用 V 验证轮换的合法性
- ✅ 块 150 → 200: 用 V' 验证
- ✅ 块 200 → 250: 继续用 V' 验证，保持状态同步

### 3. 防止长程攻击（Long Range Attack）

**攻击场景**：如果只验证单个块

```
真实链：  B₁ → B₂ → ... → B₁₀₀ → B₁₀₁ (你的交易) → B₁₀₂ → B₂₀₀
                                   ↑
                                 只验证这个块
                                   
攻击链：  B₁ → B₂ → ... → B₁₀₀ → B'₁₀₁ (伪造交易) 
                                   ↑
                              攻击者用旧私钥伪造
```

**攻击者策略**：
1. 获取历史上某个时期的验证器私钥（已轮换出去）
2. 用这些旧私钥伪造一个"块 101"
3. 如果你只验证单个块，无法区分真假

**连续验证阻止攻击**：
- 从已知的可信块（如块 50）开始
- 必须验证每一个块的签名和验证器轮换
- 攻击者无法在中间插入伪造块，因为父哈希不匹配

### 4. zkBridge 的具体实现

#### Protocol 1: Block Header Relay Network

```python
def RelayNextHeader(LCS_r-1, blkH_r-1):
    """
    持续中继下一个块头
    """
    # 1. 从多个全节点获取下一个块
    blkH_r = contact_full_nodes_for_next_block(blkH_r-1)
    
    # 2. 生成 ZK 证明，证明 blkH_r 是 blkH_r-1 的合法后继
    π = generate_zkp_proof(
        statement: LightCC(LCS_r-1, blkH_r-1, blkH_r) == True
    )
    
    # 3. 发送到目标链的 updater contract
    send_to_updater_contract(π, blkH_r, blkH_r-1)
```

#### 为什么是"持续"中继？

1. **保持状态同步**：Light Client State (LCS) 包含当前验证器集合，必须持续更新
2. **构建信任链**：每个新块的可信度依赖于前一个块
3. **应用可以查询任意块**：不同的跨链应用可能需要不同时间点的块头

## 具体例子：跨链代币转移

### 场景设置
- 源链（C₁）：Cosmos
- 目标链（C₂）：Ethereum
- 用户在块 1000 锁定代币

### 如果只中继块 1000 会发生什么？

#### ❌ 错误方案：只中继交易块
```
Ethereum 上的 Updater Contract 状态：
- headerDAG = {block_1000}  // 只有一个孤立的块
- LCS = ？？？               // 不知道当前验证器集合

问题：
1. 无法验证 block_1000 的签名（不知道验证器集合）
2. 无法验证这个块确实在主链上
3. 无法防止分叉攻击（attacker 可以提交分叉上的块）
```

#### ✅ 正确方案：持续中继
```
初始状态（部署时）：
- headerDAG = {genesis_block}
- LCS = {initial_validators: V₀}

持续同步：
Block 1 → 2 → ... → 999 → 1000 (用户交易) → 1001 → ...

Ethereum 上的 Updater Contract 状态：
- headerDAG = {完整的区块头 DAG}
- LCS = {current_validators: V_current, 历史验证器轮换记录}

验证流程：
1. ✅ 知道块 1000 的验证器集合（从 LCS 获取）
2. ✅ 可以验证块 1000 的 2/3+ 签名
3. ✅ 可以验证块 1000 在主链上（通过 parent_hash 链）
4. ✅ 可以抵抗分叉攻击（只接受最长链）
```

## zkBridge 的优化策略

虽然需要中继所有块，但 zkBridge 有优化：

### 1. 批量证明（Batching）
```
不是为每个块生成一个证明，而是：
证明：LightCC(LCS₉₉, blk₉₉, blk₁₀₀) ∧ 
      LightCC(LCS₁₀₀, blk₁₀₀, blk₁₀₁) ∧ 
      ...
      LightCC(LCS₁₉₉, blk₁₉₉, blk₂₀₀)

一个 ZK 证明覆盖 100 个块的验证
```

### 2. deVirgo 分布式加速
```
对于包含 N 个验证器签名的块：
- 传统方案：验证 N 个签名需要 O(N) 时间
- deVirgo：将 N 个签名分配给 M 个机器，时间 O(N/M)
- 线性加速比
```

### 3. 递归压缩（Recursive Compression）
```
Layer 1: deVirgo 证明（大，但快）
         ↓
Layer 2: Groth16 压缩（小，适合链上验证）

链上验证成本：从 ~80M gas 降到 ~230K gas
```

## 与其他方案对比

### 传统方案：委员会多签
```
只需中继交易块：
- 委员会签名块 1000
- 目标链验证委员会签名

问题：
- ❌ 需要信任委员会（不作恶）
- ❌ 委员会可以被攻破（Ronin 攻击：9个验证器被黑5个）
```

### zkBridge 方案：数学证明
```
需要中继所有块：
- 证明每个块都被源链的验证器集合正确签名
- 目标链验证 ZK 证明

优势：
- ✅ 无需信任任何第三方
- ✅ 继承源链的安全性
- ✅ 抗审查（任何人都可以运行 relayer）
```

## 总结

### 为什么必须收集多个块？

1. **共识协议要求**：验证一个块必须知道当时的验证器集合
2. **验证器轮换**：验证器集合动态变化，必须跟踪轮换过程
3. **防止长程攻击**：连续的区块链提供了不可伪造的历史
4. **构建信任链**：每个块的信任建立在前一个块之上
5. **支持多应用**：不同应用可能查询不同时间点的状态

### 这不是效率问题，而是安全必需

- zkBridge 的目标是**无需信任的跨链桥**
- "只收集交易块"意味着**必须信任某个第三方**告诉你验证器集合
- **持续同步所有块**是**零信任**的代价，但通过 ZK 证明大幅降低了成本

### 关键洞察

```
传统跨链桥的取舍：
  效率高（只验证交易块）
    ↓
  必须信任委员会
    ↓
  安全性降低（委员会可被攻破）

zkBridge 的取舍：
  需要同步所有块（看似低效）
    ↓
  使用 ZK 证明压缩验证成本
    ↓
  达到：高效率 + 零信任 + 高安全性
```

**最终答案**：Relayer 必须收集所有块，因为这是实现**无需信任的跨链验证**的唯一方式。通过 deVirgo + 递归压缩，zkBridge 将这个"必需的成本"优化到了可接受的范围。
