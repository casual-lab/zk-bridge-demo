# Solana ↔ Ethereum Bridge with SP1 zkVM

基于零知识证明的 Solana 和 Ethereum 双向跨链桥实现。

## 项目结构

```
solana-eth-bridge/
├── sp1-programs/           # SP1 zkVM 证明程序
│   ├── solana-verifier/    # 验证 Solana 区块
│   └── eth-verifier/       # 验证 Ethereum 区块
├── solana-program/         # Solana Anchor 程序
├── ethereum/               # Ethereum 智能合约 (Hardhat)
├── relayer/                # Rust 中继服务
└── scripts/                # 部署和测试脚本
```

## 快速开始

### 前置要求

- Rust 1.75+
- Node.js 18+
- Solana CLI 1.18+
- Anchor 0.30+
- SP1 SDK

### 安装

```bash
# 1. 安装 Solana
sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"

# 2. 安装 Anchor
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install latest
avm use latest

# 3. 安装 SP1
curl -L https://sp1.succinct.xyz | bash
sp1up
cargo prove --version

# 4. 安装 Node 依赖
npm install
```

### 本地测试

```bash
# Terminal 1: 启动 Solana 测试验证器
./scripts/start-solana.sh

# Terminal 2: 启动 Hardhat 网络
./scripts/start-hardhat.sh

# Terminal 3: 运行测试
./scripts/test-all.sh
```

## 开发路线图

- [x] Phase 0: 项目初始化
- [ ] Phase 1: SP1 Solana 验证器
- [ ] Phase 2: Ethereum Updater 合约
- [ ] Phase 3: Solana Bridge Program
- [ ] Phase 4: Relayer 实现
- [ ] Phase 5: Token Bridge 应用
- [ ] Phase 6: 集成测试

## 文档

详细设计文档见: `/workspace/docs/5-Solana-ETH-Bridge项目设计.md`

## License

MIT
