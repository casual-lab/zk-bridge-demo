# Solana ↔ Ethereum 跨链桥 (SP1 zkVM)

使用 SP1 zkVM 实现 Solana 和 Ethereum 之间的零信任双向跨链桥。

## 项目结构

```
solana-eth-bridge/
├── sp1-programs/           # SP1 zkVM 验证程序
│   ├── solana-verifier/    # Solana 区块验证器
│   └── eth-verifier/       # Ethereum 区块验证器
├── ethereum/               # Ethereum 智能合约
│   ├── contracts/
│   │   └── SolanaUpdater.sol
│   ├── scripts/
│   │   └── deploy.js
│   └── hardhat.config.js
├── solana-bridge/          # Solana Anchor 程序
│   └── programs/
│       └── solana-bridge/
│           └── src/lib.rs
├── relayer/                # 中继服务
│   ├── src/main.rs
│   └── config.toml
└── start-test-env.sh       # 启动脚本
```

## 核心组件

### 1. SP1 zkVM 程序

#### Solana 验证器 (`sp1-programs/solana-verifier`)
- ✅ 验证 Solana Tower BFT 共识
- ✅ Ed25519 签名验证（使用 SP1 precompile）
- ✅ 超过 2/3 验证器阈值检查
- ✅ 确认深度验证（防止分叉攻击）

#### Ethereum 验证器 (`sp1-programs/eth-verifier`)
- ✅ 验证 Ethereum 区块连续性
- ✅ 区块号和时间戳检查
- 🔄 简化版（本地测试网）

### 2. Ethereum 合约 (`ethereum/contracts`)

#### SolanaUpdater.sol
- ✅ 存储 Solana 区块头
- ✅ SP1 证明验证
- ✅ 确认深度检查
- ✅ Merkle 证明验证接口
- 🔄 批量更新支持

### 3. Solana 程序 (`solana-bridge`)

#### Bridge Program (Anchor)
- ✅ 存储 Ethereum 区块头
- ✅ SP1 证明验证
- ✅ 跨链消息执行
- ✅ 确认深度检查

### 4. Relayer (`relayer`)

#### 双向中继服务
- ✅ Solana 区块监控
- ✅ Ethereum 区块监控
- ✅ SP1 证明生成
- ✅ 交易提交
- 🔄 待实现具体逻辑

## 快速开始

### 环境要求

- Rust 1.75+
- Node.js 18+
- Solana CLI 1.18+
- Anchor 0.30+
- SP1 SDK 1.0+

### 安装依赖

```bash
# 1. 安装 Rust 依赖
cd sp1-programs/solana-verifier && cargo build
cd ../eth-verifier && cargo build

# 2. 安装 Ethereum 依赖
cd ../../ethereum && npm install

# 3. 构建 Solana 程序
cd ../solana-bridge && anchor build

# 4. 安装 Relayer 依赖
cd ../relayer && cargo build
```

### 启动测试环境

```bash
# 使用自动化脚本
./start-test-env.sh

# 或手动启动各组件
# Terminal 1: Solana 测试验证器
solana-test-validator --reset

# Terminal 2: Hardhat 本地网络
cd ethereum && npx hardhat node

# Terminal 3: 部署合约
cd ethereum && npx hardhat run scripts/deploy.js --network localhost

# Terminal 4: 部署 Solana 程序
cd solana-bridge && anchor deploy

# Terminal 5: 启动 Relayer
cd relayer && cargo run
```

## 配置说明

### Relayer 配置 (`relayer/config.toml`)

```toml
[solana]
rpc_url = "http://localhost:8899"
min_confirmations = 32              # 防止分叉：32 确认 (~13 秒)
poll_interval_ms = 400

[ethereum]
rpc_url = "http://localhost:8545"
min_confirmations = 12              # 本地测试：12 确认
poll_interval_ms = 12000            # 生产环境建议 64 确认

[sp1]
enable_groth16 = true               # 启用 Groth16 压缩
prove_timeout_secs = 300
```

## 安全特性

### ✅ 已实现

1. **确认深度保护**
   - Solana: 32 确认（~13 秒）
   - Ethereum: 12-64 确认（本地/生产）
   - 防止临时分叉攻击

2. **零知识证明**
   - SP1 zkVM (STARK + Groth16)
   - Ed25519 precompile 加速
   - 链上验证成本低

3. **共识验证**
   - Solana: Tower BFT 签名验证
   - Ethereum: 区块连续性验证

### 🔄 待完善

1. **重组检测**
   - 监控链重组事件
   - 自动回滚机制

2. **挑战期机制**
   - 乐观更新 + 欺诈证明
   - 经济激励模型

## 开发路线图

### Phase 1: 基础实现 ✅
- [x] SP1 验证程序
- [x] Ethereum 合约
- [x] Solana 程序
- [x] Relayer 框架

### Phase 2: 功能完善 🔄
- [ ] 实现 Relayer 核心逻辑
- [ ] 集成 SP1 SDK
- [ ] 端到端测试
- [ ] 部署脚本优化

### Phase 3: 安全加固
- [ ] 审计和测试
- [ ] 重组处理机制
- [ ] 监控和告警
- [ ] 性能优化

### Phase 4: 生产就绪
- [ ] 主网部署
- [ ] 文档完善
- [ ] 用户界面
- [ ] 运维工具

## 测试

```bash
# 测试 SP1 程序
cd sp1-programs/solana-verifier && cargo test
cd ../eth-verifier && cargo test

# 测试 Ethereum 合约
cd ethereum && npx hardhat test

# 测试 Solana 程序
cd solana-bridge && anchor test

# 测试 Relayer
cd relayer && cargo test
```

## 文档

详细设计文档位于 `/workspace/docs/`:

1. [1-zkBridge论文解析.md](../docs/1-zkBridge论文解析.md)
2. [2-Virgo协议详解.md](../docs/2-Virgo协议详解.md)
3. [3-为什么zkBridge需要收集多个块.md](../docs/3-为什么zkBridge需要收集多个块.md)
4. [4-zkBridge的持续追踪机制.md](../docs/4-zkBridge的持续追踪机制.md)
5. [5-Solana-ETH-Bridge项目设计.md](../docs/5-Solana-ETH-Bridge项目设计.md)
6. [6-处理链重组和临时分叉.md](../docs/6-处理链重组和临时分叉.md)

## 许可证

MIT

## 贡献

欢迎提交 Issue 和 Pull Request！

## 免责声明

⚠️ **本项目仅用于学习和研究目的，未经充分审计，请勿在生产环境使用！**
