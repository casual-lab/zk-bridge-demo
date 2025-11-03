#!/bin/bash

set -e

echo "🔨 Building SP1 Programs..."
echo ""

# 构建 Solana 验证器
echo "📦 Building solana-verifier..."
cd /workspace/solana-eth-bridge/sp1-programs/solana-verifier
cargo prove build
echo "✅ solana-verifier built successfully"
echo ""

# 构建 Ethereum 验证器
echo "📦 Building eth-verifier..."
cd /workspace/solana-eth-bridge/sp1-programs/eth-verifier
cargo prove build
echo "✅ eth-verifier built successfully"
echo ""

echo "🎉 All SP1 programs built!"
echo ""
echo "ELF files location:"
echo "  - solana-verifier: sp1-programs/solana-verifier/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/solana-verifier"
echo "  - eth-verifier: sp1-programs/eth-verifier/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eth-verifier"
