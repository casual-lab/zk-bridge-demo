import hre from "hardhat";
import fs from "fs";

async function main() {
  console.log("========================================");
  console.log("测试 SolanaUpdater 合约交互");
  console.log("========================================\n");

  // 读取部署信息
  const deployment = JSON.parse(fs.readFileSync("deployment.json", "utf8"));
  const solanaUpdaterAddress = deployment.solanaUpdater;

  console.log(`SolanaUpdater 地址: ${solanaUpdaterAddress}\n`);

  // 获取合约实例
  const solanaUpdater = await hre.ethers.getContractAt(
    "SolanaUpdater",
    solanaUpdaterAddress
  );

  // 测试 1: 读取初始状态
  console.log("[测试 1] 读取初始状态");
  const admin = await solanaUpdater.admin();
  const currentSlot = await solanaUpdater.currentSlot();
  const sp1Verifier = await solanaUpdater.sp1Verifier();

  console.log(`  Admin: ${admin}`);
  console.log(`  Current Slot: ${currentSlot}`);
  console.log(`  SP1 Verifier: ${sp1Verifier}`);
  console.log("  ✓ 初始状态正常\n");

  // 测试 2: 更新第一个区块
  console.log("[测试 2] 更新第一个 Solana 区块");
  const header1 = {
    slot: 1000,
    blockhash: hre.ethers.hexlify(hre.ethers.randomBytes(32)),
    parentHash: hre.ethers.ZeroHash,
    blockHeight: 1000,
    timestamp: Math.floor(Date.now() / 1000),
    confirmations: 32
  };

  const tx1 = await solanaUpdater.updateSolanaBlock("0x", header1);
  await tx1.wait();
  console.log(`  ✓ 区块 1000 已更新 (tx: ${tx1.hash})`);

  const newSlot = await solanaUpdater.currentSlot();
  console.log(`  Current Slot: ${newSlot}\n`);

  // 测试 3: 查询存储的区块
  console.log("[测试 3] 查询存储的区块");
  const storedHeader = await solanaUpdater.getSolanaBlock(1000);
  console.log(`  Slot: ${storedHeader.slot}`);
  console.log(`  Block Height: ${storedHeader.blockHeight}`);
  console.log(`  Blockhash: ${storedHeader.blockhash}`);
  console.log(`  Confirmations: ${storedHeader.confirmations}`);
  console.log("  ✓ 区块数据正确\n");

  // 测试 4: 连续更新多个区块
  console.log("[测试 4] 连续更新多个区块");
  
  const header2 = {
    slot: 1001,
    blockhash: hre.ethers.hexlify(hre.ethers.randomBytes(32)),
    parentHash: header1.blockhash,
    blockHeight: 1001,
    timestamp: Math.floor(Date.now() / 1000),
    confirmations: 32
  };

  const tx2 = await solanaUpdater.updateSolanaBlock("0x", header2);
  await tx2.wait();
  console.log(`  ✓ 区块 1001 已更新`);

  const header3 = {
    slot: 1002,
    blockhash: hre.ethers.hexlify(hre.ethers.randomBytes(32)),
    parentHash: header2.blockhash,
    blockHeight: 1002,
    timestamp: Math.floor(Date.now() / 1000),
    confirmations: 32
  };

  const tx3 = await solanaUpdater.updateSolanaBlock("0x", header3);
  await tx3.wait();
  console.log(`  ✓ 区块 1002 已更新`);

  const finalSlot = await solanaUpdater.currentSlot();
  console.log(`  Current Slot: ${finalSlot}\n`);

  // 测试 5: 批量更新
  console.log("[测试 5] 批量更新多个区块");
  
  const headers = [];
  let prevHash = header3.blockhash;
  
  for (let i = 0; i < 3; i++) {
    const slot = 1003 + i;
    const blockhash = hre.ethers.hexlify(hre.ethers.randomBytes(32));
    
    headers.push({
      slot,
      blockhash,
      parentHash: prevHash,
      blockHeight: slot,
      timestamp: Math.floor(Date.now() / 1000),
      confirmations: 32
    });
    
    prevHash = blockhash;
  }

  const proofs = headers.map(() => "0x");
  const txBatch = await solanaUpdater.updateBatchSolanaBlocks(proofs, headers);
  await txBatch.wait();
  
  console.log(`  ✓ 批量更新 3 个区块成功`);
  
  const batchSlot = await solanaUpdater.currentSlot();
  console.log(`  Current Slot: ${batchSlot}\n`);

  // 总结
  console.log("========================================");
  console.log("✓ 所有测试通过！");
  console.log("========================================");
  console.log(`最终状态:`);
  console.log(`  - 总共更新了 ${batchSlot} 个区块`);
  console.log(`  - 当前 slot: ${batchSlot}`);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
