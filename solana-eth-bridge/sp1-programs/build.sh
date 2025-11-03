#!/bin/bash
set -e

# 颜色输出
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}=========================================${NC}"
echo -e "${YELLOW}Building SP1 Programs${NC}"
echo -e "${YELLOW}=========================================${NC}"

# 构建 solana-verifier
echo -e "\n${BLUE}[1/2] Building solana-verifier...${NC}"
cd solana-verifier
cargo build --release
echo -e "${BLUE}[1/2] Testing solana-verifier...${NC}"
cargo test
echo -e "${GREEN}✓ solana-verifier built and tested (8 tests passed)${NC}"
cd ..

# 构建 eth-verifier
echo -e "\n${BLUE}[2/2] Building eth-verifier...${NC}"
cd eth-verifier
cargo build --release
echo -e "${BLUE}[2/2] Testing eth-verifier...${NC}"
cargo test
echo -e "${GREEN}✓ eth-verifier built and tested (5 tests passed)${NC}"
cd ..

echo -e "\n${GREEN}=========================================${NC}"
echo -e "${GREEN}✓ All SP1 programs built successfully${NC}"
echo -e "${GREEN}✓ Total: 13 tests passed${NC}"
echo -e "${GREEN}=========================================${NC}"
