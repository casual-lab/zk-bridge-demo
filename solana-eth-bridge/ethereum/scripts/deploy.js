import hre from "hardhat";
import fs from "fs";

async function main() {
  console.log("Deploying contracts...");

  // 1. 部署 MockSP1Verifier (strictMode = false for easier testing)
  console.log("\n1. Deploying MockSP1Verifier...");
  const MockSP1Verifier = await hre.ethers.getContractFactory("MockSP1Verifier");
  const mockVerifier = await MockSP1Verifier.deploy(false);
  await mockVerifier.waitForDeployment();
  const verifierAddress = await mockVerifier.getAddress();
  console.log(`   ✓ MockSP1Verifier deployed to: ${verifierAddress}`);

  // 2. 部署 SolanaUpdater
  console.log("\n2. Deploying SolanaUpdater...");
  const SolanaUpdater = await hre.ethers.getContractFactory("SolanaUpdater");
  const solanaUpdater = await SolanaUpdater.deploy(verifierAddress);
  await solanaUpdater.waitForDeployment();
  const updaterAddress = await solanaUpdater.getAddress();
  console.log(`   ✓ SolanaUpdater deployed to: ${updaterAddress}`);
  
  // 保存地址到文件
  const deploymentInfo = {
    network: hre.network.name,
    mockSP1Verifier: verifierAddress,
    solanaUpdater: updaterAddress,
    deployedAt: new Date().toISOString(),
    note: "Using MockSP1Verifier for development. Replace with real SP1 verifier in production."
  };
  
  fs.writeFileSync(
    'deployment.json',
    JSON.stringify(deploymentInfo, null, 2)
  );
  
  console.log("\n✓ Deployment info saved to deployment.json");
  console.log("\n📝 Summary:");
  console.log(`   MockSP1Verifier: ${verifierAddress}`);
  console.log(`   SolanaUpdater:   ${updaterAddress}`);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
