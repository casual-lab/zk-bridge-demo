# Sprint 2: Ethereum 合约测试 - 快速开始

## 🎯 目标

让 Ethereum 智能合约能在 Hardhat 本地网络上部署和测试，验证所有核心功能。

## 📋 任务清单

- [ ] 2.1 编写 SolanaUpdater 合约单元测试
- [ ] 2.2 编写 EthereumUpdater 合约单元测试
- [ ] 2.3 本地部署测试
- [ ] 2.4 交互测试脚本

## ⏱️ 预计时间

2-3 天

---

## 步骤 2.1: SolanaUpdater 合约测试

### 文件: `ethereum/test/SolanaUpdater.test.js`

需要测试的功能点：

1. ✅ 合约初始化
2. ✅ 更新 Solana 区块
3. ✅ 确认数验证（≥32）
4. ✅ 区块连续性验证
5. ✅ 查询已存储的区块
6. ✅ 权限控制（只有 admin 能更新）

### 测试命令

```bash
cd /workspace/solana-eth-bridge/ethereum
npx hardhat test test/SolanaUpdater.test.js
```

### 预期结果

```
SolanaUpdater
  ✓ 应该正确初始化
  ✓ 应该能更新 Solana 区块
  ✓ 应该拒绝确认数不足的区块
  ✓ 应该验证区块连续性
  ✓ 应该能查询存储的区块
  ✓ 应该拒绝非 admin 的更新
```

---

## 步骤 2.2: EthereumUpdater 合约测试

### 文件: `ethereum/test/EthereumUpdater.test.js`

需要测试的功能点：

1. ✅ 合约初始化
2. ✅ 更新 Ethereum 区块
3. ✅ 确认数验证（≥12）
4. ✅ 区块高度递增验证
5. ✅ 查询已存储的区块

### 测试命令

```bash
cd /workspace/solana-eth-bridge/ethereum
npx hardhat test test/EthereumUpdater.test.js
```

---

## 步骤 2.3: 本地部署测试

### Terminal 1: 启动本地节点

```bash
cd /workspace/solana-eth-bridge/ethereum
npx hardhat node
```

### Terminal 2: 部署合约

```bash
cd /workspace/solana-eth-bridge/ethereum
npx hardhat run scripts/deploy.js --network localhost
```

### 预期输出

```
Deploying SolanaUpdater...
SolanaUpdater deployed to: 0x5FbDB2315678afecb367f032d93F642f64180aa3

Deploying EthereumUpdater...
EthereumUpdater deployed to: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512

✓ Deployment info saved to deployment.json
```

---

## 步骤 2.4: 交互测试脚本

### 文件: `ethereum/scripts/test-interaction.js`

测试场景：

1. 读取初始状态
2. 更新 Solana 区块
3. 更新 Ethereum 区块
4. 查询存储的区块

### 运行命令

```bash
npx hardhat run scripts/test-interaction.js --network localhost
```

---

## 🧪 完整测试流程

### 一键运行所有测试

```bash
cd /workspace/solana-eth-bridge/ethereum
npx hardhat test
```

### 自动化脚本

创建 `ethereum/test-all.sh`:

```bash
#!/bin/bash
set -e

echo "========================================="
echo "Ethereum 合约测试套件"
echo "========================================="

echo "[1/3] 运行单元测试..."
npx hardhat test

echo ""
echo "[2/3] 启动本地节点..."
npx hardhat node > /dev/null 2>&1 &
NODE_PID=$!
sleep 3

echo "[3/3] 测试部署和交互..."
npx hardhat run scripts/deploy.js --network localhost
npx hardhat run scripts/test-interaction.js --network localhost

# 清理
kill $NODE_PID

echo ""
echo "✓ 所有测试通过"
```

---

## ✅ 验收标准

Sprint 2 完成需要满足：

1. ✅ 所有单元测试通过（至少 10+ 个测试）
2. ✅ 合约能在本地网络成功部署
3. ✅ deployment.json 文件正确生成
4. ✅ 交互脚本能读写合约状态
5. ✅ 测试覆盖所有核心功能

---

## 🐛 可能遇到的问题

### 问题 1: Hardhat 未安装

```bash
cd ethereum
npm install
```

### 问题 2: 端口被占用

```bash
# 杀死占用 8545 的进程
lsof -ti:8545 | xargs kill -9
```

### 问题 3: 合约编译失败

```bash
npx hardhat clean
npx hardhat compile
```

---

最后更新: 2025-11-03
状态: 准备开始
