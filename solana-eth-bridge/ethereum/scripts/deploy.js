const hre = require("hardhat");

async function main() {
  console.log("Deploying SolanaUpdater contract...");

  // 部署 SolanaUpdater（暂时使用零地址作为 SP1 验证器）
  const SolanaUpdater = await hre.ethers.getContractFactory("SolanaUpdater");
  const solanaUpdater = await SolanaUpdater.deploy(
    "0x0000000000000000000000000000000000000000" // SP1 Verifier 地址（稍后替换）
  );

  await solanaUpdater.waitForDeployment();

  const address = await solanaUpdater.getAddress();
  console.log(`✓ SolanaUpdater deployed to: ${address}`);
  
  // 保存地址到文件
  const fs = require('fs');
  const deploymentInfo = {
    network: hre.network.name,
    solanaUpdater: address,
    deployedAt: new Date().toISOString()
  };
  
  fs.writeFileSync(
    'deployment.json',
    JSON.stringify(deploymentInfo, null, 2)
  );
  
  console.log("✓ Deployment info saved to deployment.json");
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
