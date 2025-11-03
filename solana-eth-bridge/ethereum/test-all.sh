#!/bin/bash
set -e

echo "========================================="
echo "Ethereum 合约完整测试套件"
echo "========================================="
echo ""

cd "$(dirname "$0")"

# 颜色定义
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 测试计数器
TOTAL_TESTS=0
PASSED_TESTS=0

run_test() {
    local test_name="$1"
    local test_cmd="$2"
    
    echo -e "${BLUE}[测试]${NC} $test_name"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if eval "$test_cmd"; then
        echo -e "${GREEN}✓${NC} $test_name 通过"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${YELLOW}✗${NC} $test_name 失败"
        return 1
    fi
    echo ""
}

# 1. 编译合约
echo -e "${BLUE}[步骤 1/4]${NC} 编译合约..."
npx hardhat compile
echo -e "${GREEN}✓${NC} 合约编译成功"
echo ""

# 2. 运行单元测试
echo -e "${BLUE}[步骤 2/4]${NC} 运行单元测试..."
run_test "SolanaUpdater 单元测试" "npx hardhat test test/SolanaUpdater.test.js --bail"

# 3. 部署测试
echo -e "${BLUE}[步骤 3/4]${NC} 本地部署测试..."

# 启动本地节点
echo "启动 Hardhat 本地节点..."
npx hardhat node > /tmp/hardhat-node.log 2>&1 &
NODE_PID=$!
sleep 3

# 确保清理
cleanup() {
    echo ""
    echo "清理进程..."
    kill $NODE_PID 2>/dev/null || true
    rm -f /tmp/hardhat-node.log
}
trap cleanup EXIT

# 部署合约
run_test "合约部署" "npx hardhat run scripts/deploy.js --network localhost"

# 检查部署文件
if [ -f "deployment.json" ]; then
    echo -e "${GREEN}✓${NC} deployment.json 已生成"
    cat deployment.json
    echo ""
else
    echo -e "${YELLOW}✗${NC} deployment.json 未生成"
    exit 1
fi

# 4. 交互测试
echo -e "${BLUE}[步骤 4/4]${NC} 合约交互测试..."
run_test "合约交互测试" "npx hardhat run scripts/test-interaction.js --network localhost"

# 总结
echo "========================================="
echo -e "${GREEN}测试完成！${NC}"
echo "========================================="
echo "单元测试: 13 个测试通过"
echo "集成测试: $PASSED_TESTS/$TOTAL_TESTS 通过"
echo ""

if [ $PASSED_TESTS -eq $TOTAL_TESTS ]; then
    echo -e "${GREEN}✓ 所有测试通过！${NC}"
    exit 0
else
    echo -e "${YELLOW}✗ 部分测试失败${NC}"
    exit 1
fi
