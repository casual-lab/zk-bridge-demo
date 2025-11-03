#!/bin/bash

# Solana-Ethereum Bridge 测试脚本

set -e

echo "========================================="
echo "Solana-Ethereum Bridge 本地测试环境启动"
echo "========================================="

# 1. 启动 Solana 测试验证器
echo ""
echo "[1/4] 启动 Solana 测试验证器..."
solana-test-validator --reset &
SOLANA_PID=$!
sleep 5
echo "✓ Solana 测试验证器已启动 (PID: $SOLANA_PID)"

# 2. 启动 Hardhat 本地网络
echo ""
echo "[2/4] 启动 Hardhat 本地网络..."
cd ethereum
npx hardhat node &
HARDHAT_PID=$!
sleep 5
echo "✓ Hardhat 节点已启动 (PID: $HARDHAT_PID)"
cd ..

# 3. 部署 Ethereum 合约
echo ""
echo "[3/4] 部署 Ethereum 合约..."
cd ethereum
npx hardhat run scripts/deploy.js --network localhost
cd ..

# 4. 部署 Solana 程序
echo ""
echo "[4/4] 部署 Solana 程序..."
cd solana-bridge
anchor build
anchor deploy
cd ..

echo ""
echo "========================================="
echo "✓ 所有组件已启动！"
echo "========================================="
echo ""
echo "Solana RPC: http://localhost:8899"
echo "Ethereum RPC: http://localhost:8545"
echo ""
echo "按 Ctrl+C 停止所有服务"

# 等待中断信号
trap "echo ''; echo 'Stopping services...'; kill $SOLANA_PID $HARDHAT_PID 2>/dev/null; exit" INT
wait
