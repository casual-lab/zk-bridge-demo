# EVM-Solana Bridge (Phase 2)

## 概述

这是跨链桥的 EVM 端实现（Phase 2），与 Solana 端（Phase 1.4）功能完全对称。

## 功能特性

### 核心功能

1. **桥初始化** (`initializeBridge`)
   - 设置 Solana 链 ID
   - 初始化 Relayer 费用配置（默认 0.1%，最低 0.05 USDC）

2. **代币对注册** (`registerTokenPair`)
   - 注册 ERC20 代币与 Solana SPL 代币的映射关系
   - 支持两种模式：
     - Lock/Unlock 模式（`isNativeEvm = true`）
     - Burn/Mint 模式（`isNativeEvm = false`，未来实现）

3. **代币锁定** (`lockTokens`)
   - 用户锁定 ERC20 代币，准备跨链到 Solana
   - 自动计算并扣除 Relayer 费用（百分比费用）
   - 生成唯一订单 ID
   - 发出 `TokensLocked` 事件

4. **代币解锁** (`unlockTokens`)
   - Relayer 提供 ZK 证明解锁代币
   - 重新计算 Relayer 费用（考虑最小费用）
   - 分配代币给用户和 Relayer
   - 防止重复解锁
   - 发出 `TokensUnlocked` 事件

5. **管理功能**
   - 更新 Relayer 费用配置 (`updateRelayerFee`)
   - 暂停/恢复桥 (`setPaused`)

### Relayer 费用机制（Phase 1.4 对称）

与 Solana 端完全一致：

- **锁定时**：扣除百分比费用（默认 0.1%）
- **解锁时**：使用 `max(百分比费用, 最小费用)` 分配给 Relayer
- **默认配置**：
  - 费率：10 bps (0.1%)
  - 最小费用：50,000 单位（对于 6 位小数代币 = 0.05 USDC）

### 订单状态

- `Pending`: 代币已锁定，等待跨链完成
- `Completed`: 跨链已完成，代币已解锁

**注意**：Phase 1.4 设计中取消了超时退款机制，简化了状态机。

## 合约架构

```
EVMSolanaBridge
├── 状态变量
│   ├── admin: 管理员地址
│   ├── solanaChainId: Solana 链标识
│   ├── paused: 暂停状态
│   ├── nextOrderId: 下一个订单 ID
│   ├── relayerFeeBps: Relayer 费率（基点）
│   ├── minRelayerFee: 最小 Relayer 费用
│   ├── tokenConfigs: 代币配置映射
│   ├── transferOrders: 订单映射
│   └── vaults: 代币金库余额
│
├── 数据结构
│   ├── TokenConfig: 代币对配置
│   ├── TransferOrder: 转账订单
│   └── OrderStatus: 订单状态枚举
│
├── 管理功能
│   ├── initializeBridge(): 初始化桥
│   ├── registerTokenPair(): 注册代币对
│   ├── updateRelayerFee(): 更新费用配置
│   └── setPaused(): 暂停/恢复
│
├── 用户功能
│   ├── lockTokens(): 锁定代币
│   └── unlockTokens(): 解锁代币（Relayer 调用）
│
└── 查询功能
    ├── calculateRelayerFee(): 计算费用
    ├── getTokenConfig(): 获取代币配置
    ├── getTransferOrder(): 获取订单详情
    └── getVaultBalance(): 获取金库余额
```

## 与 Solana 端的对称性

| 功能 | Solana (Rust) | EVM (Solidity) |
|------|--------------|----------------|
| 桥初始化 | `initialize_bridge` | `initializeBridge` |
| 注册代币对 | `register_token_pair` | `registerTokenPair` |
| 初始化金库 | `initialize_vault` | (自动管理) |
| 锁定代币 | `lock_tokens` | `lockTokens` |
| 解锁代币 | `unlock_tokens` | `unlockTokens` |
| 费用计算 | `calculate_relayer_fee` | `calculateRelayerFee` |
| 订单状态 | `OrderStatus::Pending/Completed` | `OrderStatus.Pending/Completed` |
| 费用配置 | 10 bps, 50000 min | 10 bps, 50000 min |

## 测试

### 运行测试

```bash
npm test
```

### 测试覆盖

✅ 16/16 测试通过：

1. **初始化**
   - 桥初始化正确性
   - 防止重复初始化

2. **代币注册**
   - 注册代币对
   - 防止重复注册

3. **代币锁定**
   - 正常锁定流程
   - 拒绝零金额
   - 拒绝未注册代币
   - 小额转账费用计算

4. **代币解锁**
   - 正常解锁流程（含 ZK 证明验证）
   - 拒绝无效证明（零哈希）
   - 防止重复解锁
   - 强制执行最小 Relayer 费用

5. **管理功能**
   - 更新 Relayer 费用
   - 暂停/恢复桥
   - 暂停时拒绝锁定

6. **查询功能**
   - 费用计算正确性

### Gas 成本

| 操作 | Gas 消耗 |
|------|----------|
| `initializeBridge` | ~52,065 |
| `registerTokenPair` | ~95,538 |
| `lockTokens` | ~256,194 |
| `unlockTokens` | ~170,183 |
| `updateRelayerFee` | ~35,294 |
| `setPaused` | ~29,776 |

## 部署

### 本地测试网

```bash
npx hardhat compile
npx hardhat test
```

### 部署到测试网（未来）

```bash
# 配置 hardhat.config.js 中的网络设置
npx hardhat run scripts/deploy.js --network arbitrum-sepolia
```

## 安全特性

1. **重入保护**：使用 OpenZeppelin 的 `ReentrancyGuard`
2. **访问控制**：使用 `Ownable` 限制管理功能
3. **状态验证**：
   - 订单状态检查防止重复解锁
   - 代币注册检查
   - 暂停状态检查
4. **安全转账**：使用 `SafeERC20` 处理代币转账
5. **溢出保护**：Solidity 0.8+ 内置溢出检查

## 未来增强（Phase 3+）

1. **真实 ZK 证明验证**
   - 集成 SP1 zkVM 验证器
   - 替换当前的 mock 验证逻辑

2. **跨链状态同步**
   - 实现轻客户端状态根验证
   - 自动化 Relayer 服务

3. **Burn/Mint 模式**
   - 支持外来代币的销毁/铸造
   - 与 Lock/Unlock 并行运行

4. **多签管理**
   - 引入多签机制管理关键操作
   - 提升去中心化程度

5. **跨链消息传递**
   - 扩展到支持任意消息跨链
   - 不仅限于代币转账

## 依赖

- Solidity: ^0.8.20
- OpenZeppelin Contracts: ^5.4.0
- Hardhat: ^2.22.0
- Ethers.js: ^6.x

## 许可

MIT
