const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("EVMSolanaBridge", function () {
  let bridge;
  let mockToken;
  let owner;
  let user;
  let relayer;
  
  const SOLANA_CHAIN_ID = ethers.id("solana-mainnet");
  const DECIMALS = 6; // USDC-like token
  const INITIAL_SUPPLY = ethers.parseUnits("1000000", DECIMALS); // 1M tokens
  
  beforeEach(async function () {
    [owner, user, relayer] = await ethers.getSigners();
    
    // Deploy MockERC20
    const MockERC20 = await ethers.getContractFactory("MockERC20");
    mockToken = await MockERC20.deploy("USD Coin", "USDC", DECIMALS);
    await mockToken.waitForDeployment();
    
    // Deploy Bridge
    const EVMSolanaBridge = await ethers.getContractFactory("EVMSolanaBridge");
    bridge = await EVMSolanaBridge.deploy();
    await bridge.waitForDeployment();
    
    // Mint tokens to user
    await mockToken.mint(user.address, INITIAL_SUPPLY);
    
    console.log("✅ Contracts deployed");
    console.log("   Bridge:", await bridge.getAddress());
    console.log("   MockToken:", await mockToken.getAddress());
    console.log("   User balance:", ethers.formatUnits(await mockToken.balanceOf(user.address), DECIMALS), "USDC");
  });
  
  describe("Initialization", function () {
    it("Should initialize bridge correctly", async function () {
      await bridge.initializeBridge(SOLANA_CHAIN_ID);
      
      expect(await bridge.solanaChainId()).to.equal(SOLANA_CHAIN_ID);
      expect(await bridge.admin()).to.equal(owner.address);
      expect(await bridge.paused()).to.equal(false);
      expect(await bridge.nextOrderId()).to.equal(1);
      expect(await bridge.relayerFeeBps()).to.equal(10); // 0.1%
      expect(await bridge.minRelayerFee()).to.equal(50000); // 0.05 USDC
      
      console.log("✅ Bridge initialized successfully");
      console.log("   Relayer fee: 0.1% (10 bps)");
      console.log("   Min relayer fee: 0.05 USDC (50000 units)");
    });
    
    it("Should not allow double initialization", async function () {
      await bridge.initializeBridge(SOLANA_CHAIN_ID);
      
      await expect(
        bridge.initializeBridge(SOLANA_CHAIN_ID)
      ).to.be.revertedWith("Already initialized");
    });
  });
  
  describe("Token Registration", function () {
    beforeEach(async function () {
      await bridge.initializeBridge(SOLANA_CHAIN_ID);
    });
    
    it("Should register token pair", async function () {
      const solanaMint = ethers.randomBytes(32);
      
      await bridge.registerTokenPair(
        await mockToken.getAddress(),
        solanaMint,
        true // isNativeEvm
      );
      
      const config = await bridge.getTokenConfig(await mockToken.getAddress());
      expect(config.evmToken).to.equal(await mockToken.getAddress());
      expect(config.solanaMint).to.equal(ethers.hexlify(solanaMint));
      expect(config.isNativeEvm).to.equal(true);
      expect(config.totalLocked).to.equal(0);
      
      console.log("✅ Token pair registered successfully");
    });
    
    it("Should not allow duplicate registration", async function () {
      const solanaMint = ethers.randomBytes(32);
      
      await bridge.registerTokenPair(
        await mockToken.getAddress(),
        solanaMint,
        true
      );
      
      await expect(
        bridge.registerTokenPair(
          await mockToken.getAddress(),
          solanaMint,
          true
        )
      ).to.be.revertedWith("Token already registered");
    });
  });
  
  describe("Lock Tokens", function () {
    const LOCK_AMOUNT = ethers.parseUnits("1", DECIMALS); // 1 USDC
    let solanaMint;
    let recipient;
    
    beforeEach(async function () {
      await bridge.initializeBridge(SOLANA_CHAIN_ID);
      
      solanaMint = ethers.randomBytes(32);
      recipient = ethers.randomBytes(32); // Solana address (32 bytes)
      
      await bridge.registerTokenPair(
        await mockToken.getAddress(),
        solanaMint,
        true
      );
      
      // Approve bridge to spend user's tokens
      await mockToken.connect(user).approve(
        await bridge.getAddress(),
        LOCK_AMOUNT
      );
    });
    
    it("Should lock tokens correctly", async function () {
      const userBalanceBefore = await mockToken.balanceOf(user.address);
      
      const tx = await bridge.connect(user).lockTokens(
        await mockToken.getAddress(),
        LOCK_AMOUNT,
        recipient
      );
      
      const receipt = await tx.wait();
      const event = receipt.logs.find(log => {
        try {
          return bridge.interface.parseLog(log).name === "TokensLocked";
        } catch {
          return false;
        }
      });
      
      const parsedEvent = bridge.interface.parseLog(event);
      const orderId = parsedEvent.args.orderId;
      
      // Check order details
      const order = await bridge.getTransferOrder(orderId);
      expect(order.orderId).to.equal(orderId);
      expect(order.user).to.equal(user.address);
      expect(order.sourceChain).to.equal(1); // EVM
      expect(order.tokenConfig).to.equal(await mockToken.getAddress());
      expect(order.recipient).to.equal(ethers.hexlify(recipient));
      expect(order.status).to.equal(0); // Pending
      
      // Check fee calculation
      const expectedFee = LOCK_AMOUNT * 10n / 10000n; // 0.1%
      const expectedLocked = LOCK_AMOUNT - expectedFee;
      
      expect(order.amount).to.equal(expectedLocked);
      expect(order.relayerFee).to.equal(expectedFee);
      
      // Check balances
      const userBalanceAfter = await mockToken.balanceOf(user.address);
      expect(userBalanceAfter).to.equal(userBalanceBefore - LOCK_AMOUNT);
      
      const vaultBalance = await bridge.getVaultBalance(await mockToken.getAddress());
      expect(vaultBalance).to.equal(expectedLocked);
      
      console.log("✅ Tokens locked successfully");
      console.log("   Order ID:", orderId.toString());
      console.log("   Amount locked:", ethers.formatUnits(expectedLocked, DECIMALS), "USDC");
      console.log("   Relayer fee:", ethers.formatUnits(expectedFee, DECIMALS), "USDC");
      console.log("   Vault balance:", ethers.formatUnits(vaultBalance, DECIMALS), "USDC");
    });
    
    it("Should reject zero amount", async function () {
      await expect(
        bridge.connect(user).lockTokens(
          await mockToken.getAddress(),
          0,
          recipient
        )
      ).to.be.revertedWithCustomError(bridge, "InvalidAmount");
    });
    
    it("Should reject unregistered token", async function () {
      const MockERC20 = await ethers.getContractFactory("MockERC20");
      const unregisteredToken = await MockERC20.deploy("Other", "OTH", 18);
      await unregisteredToken.waitForDeployment();
      
      await expect(
        bridge.connect(user).lockTokens(
          await unregisteredToken.getAddress(),
          LOCK_AMOUNT,
          recipient
        )
      ).to.be.revertedWithCustomError(bridge, "TokenNotRegistered");
    });
    
    it("Should respect minimum fee", async function () {
      // Lock small amount that would result in fee < minimum
      const smallAmount = ethers.parseUnits("0.01", DECIMALS); // 0.01 USDC
      
      await mockToken.connect(user).approve(
        await bridge.getAddress(),
        smallAmount
      );
      
      const tx = await bridge.connect(user).lockTokens(
        await mockToken.getAddress(),
        smallAmount,
        recipient
      );
      
      const receipt = await tx.wait();
      const event = receipt.logs.find(log => {
        try {
          return bridge.interface.parseLog(log).name === "TokensLocked";
        } catch {
          return false;
        }
      });
      
      const parsedEvent = bridge.interface.parseLog(event);
      const orderId = parsedEvent.args.orderId;
      const order = await bridge.getTransferOrder(orderId);
      
      // Fee should be 0.1% = 10 units, but minimum is 50000
      // So fee deduction during lock is still percentage-based
      const expectedFee = smallAmount * 10n / 10000n;
      expect(order.relayerFee).to.equal(expectedFee);
      
      console.log("✅ Small amount locked with percentage fee");
      console.log("   Amount:", ethers.formatUnits(smallAmount, DECIMALS), "USDC");
      console.log("   Fee deducted:", ethers.formatUnits(expectedFee, DECIMALS), "USDC");
    });
  });
  
  describe("Unlock Tokens", function () {
    const LOCK_AMOUNT = ethers.parseUnits("1", DECIMALS);
    let orderId;
    let solanaMint;
    let recipient;
    
    beforeEach(async function () {
      await bridge.initializeBridge(SOLANA_CHAIN_ID);
      
      solanaMint = ethers.randomBytes(32);
      recipient = ethers.randomBytes(32);
      
      await bridge.registerTokenPair(
        await mockToken.getAddress(),
        solanaMint,
        true
      );
      
      // Lock tokens first
      await mockToken.connect(user).approve(
        await bridge.getAddress(),
        LOCK_AMOUNT
      );
      
      const tx = await bridge.connect(user).lockTokens(
        await mockToken.getAddress(),
        LOCK_AMOUNT,
        recipient
      );
      
      const receipt = await tx.wait();
      const event = receipt.logs.find(log => {
        try {
          return bridge.interface.parseLog(log).name === "TokensLocked";
        } catch {
          return false;
        }
      });
      
      orderId = bridge.interface.parseLog(event).args.orderId;
      
      console.log("✅ Setup: Tokens locked, order ID:", orderId.toString());
    });
    
    it("Should unlock tokens with valid proof", async function () {
      const mockProofHash = ethers.randomBytes(32);
      
      const orderBefore = await bridge.getTransferOrder(orderId);
      const userBalanceBefore = await mockToken.balanceOf(user.address);
      const relayerBalanceBefore = await mockToken.balanceOf(relayer.address);
      const vaultBalanceBefore = await bridge.getVaultBalance(await mockToken.getAddress());
      
      console.log("Before unlock:");
      console.log("   User balance:", ethers.formatUnits(userBalanceBefore, DECIMALS), "USDC");
      console.log("   Vault balance:", ethers.formatUnits(vaultBalanceBefore, DECIMALS), "USDC");
      console.log("   Order status:", orderBefore.status === 0n ? "Pending" : "Completed");
      
      // Relayer unlocks the tokens
      const tx = await bridge.connect(relayer).unlockTokens(orderId, mockProofHash);
      await tx.wait();
      
      const orderAfter = await bridge.getTransferOrder(orderId);
      const userBalanceAfter = await mockToken.balanceOf(user.address);
      const relayerBalanceAfter = await mockToken.balanceOf(relayer.address);
      const vaultBalanceAfter = await bridge.getVaultBalance(await mockToken.getAddress());
      
      // Check order status
      expect(orderAfter.status).to.equal(1); // Completed
      expect(orderAfter.proofHash).to.equal(ethers.hexlify(mockProofHash));
      expect(orderAfter.completedBy).to.equal(relayer.address);
      expect(orderAfter.completedAt).to.be.gt(0);
      
      // Calculate expected amounts (matches Solana logic)
      const totalAmount = orderBefore.amount;
      const minFee = await bridge.minRelayerFee();
      const calculatedFee = totalAmount * 10n / 10000n;
      const relayerReward = calculatedFee < minFee ? minFee : calculatedFee;
      const userAmount = totalAmount - relayerReward;
      
      // Check balances
      expect(userBalanceAfter - userBalanceBefore).to.equal(userAmount);
      expect(relayerBalanceAfter - relayerBalanceBefore).to.equal(relayerReward);
      expect(vaultBalanceAfter).to.equal(0n);
      
      console.log("✅ Tokens unlocked successfully");
      console.log("   User received:", ethers.formatUnits(userAmount, DECIMALS), "USDC");
      console.log("   Relayer reward:", ethers.formatUnits(relayerReward, DECIMALS), "USDC");
      console.log("   Order status:", orderAfter.status === 1n ? "Completed" : "Pending");
      console.log("   User balance after:", ethers.formatUnits(userBalanceAfter, DECIMALS), "USDC");
      console.log("   Vault balance after:", ethers.formatUnits(vaultBalanceAfter, DECIMALS), "USDC");
    });
    
    it("Should reject invalid proof (zero hash)", async function () {
      const zeroProof = ethers.ZeroHash;
      
      await expect(
        bridge.connect(relayer).unlockTokens(orderId, zeroProof)
      ).to.be.revertedWithCustomError(bridge, "InvalidProof");
    });
    
    it("Should reject double unlock", async function () {
      const mockProofHash = ethers.randomBytes(32);
      
      // First unlock
      await bridge.connect(relayer).unlockTokens(orderId, mockProofHash);
      
      // Second unlock should fail
      await expect(
        bridge.connect(relayer).unlockTokens(orderId, mockProofHash)
      ).to.be.revertedWithCustomError(bridge, "OrderNotPending");
    });
    
    it("Should enforce minimum relayer fee on unlock", async function () {
      // The locked amount already has fee deducted
      // On unlock, we ensure relayer gets at least minRelayerFee
      const mockProofHash = ethers.randomBytes(32);
      
      const relayerBalanceBefore = await mockToken.balanceOf(relayer.address);
      
      await bridge.connect(relayer).unlockTokens(orderId, mockProofHash);
      
      const relayerBalanceAfter = await mockToken.balanceOf(relayer.address);
      const relayerReward = relayerBalanceAfter - relayerBalanceBefore;
      
      const minFee = await bridge.minRelayerFee();
      expect(relayerReward).to.be.gte(minFee);
      
      console.log("✅ Minimum relayer fee enforced");
      console.log("   Relayer reward:", ethers.formatUnits(relayerReward, DECIMALS), "USDC");
      console.log("   Minimum fee:", ethers.formatUnits(minFee, DECIMALS), "USDC");
    });
  });
  
  describe("Admin Functions", function () {
    beforeEach(async function () {
      await bridge.initializeBridge(SOLANA_CHAIN_ID);
    });
    
    it("Should update relayer fee", async function () {
      const newFeeBps = 20; // 0.2%
      const newMinFee = ethers.parseUnits("0.1", DECIMALS);
      
      await bridge.updateRelayerFee(newFeeBps, newMinFee);
      
      expect(await bridge.relayerFeeBps()).to.equal(newFeeBps);
      expect(await bridge.minRelayerFee()).to.equal(newMinFee);
      
      console.log("✅ Relayer fee updated");
      console.log("   New fee:", newFeeBps / 100, "%");
      console.log("   New min fee:", ethers.formatUnits(newMinFee, DECIMALS), "USDC");
    });
    
    it("Should pause and unpause bridge", async function () {
      await bridge.setPaused(true);
      expect(await bridge.paused()).to.equal(true);
      
      await bridge.setPaused(false);
      expect(await bridge.paused()).to.equal(false);
      
      console.log("✅ Bridge pause/unpause works");
    });
    
    it("Should reject lock when paused", async function () {
      const solanaMint = ethers.randomBytes(32);
      const recipient = ethers.randomBytes(32);
      
      await bridge.registerTokenPair(
        await mockToken.getAddress(),
        solanaMint,
        true
      );
      
      await bridge.setPaused(true);
      
      await mockToken.connect(user).approve(
        await bridge.getAddress(),
        ethers.parseUnits("1", DECIMALS)
      );
      
      await expect(
        bridge.connect(user).lockTokens(
          await mockToken.getAddress(),
          ethers.parseUnits("1", DECIMALS),
          recipient
        )
      ).to.be.revertedWithCustomError(bridge, "BridgeIsPaused");
    });
  });
  
  describe("View Functions", function () {
    it("Should calculate relayer fee correctly", async function () {
      // Test percentage fee
      const amount1 = ethers.parseUnits("100", DECIMALS);
      const fee1 = await bridge.calculateRelayerFee(amount1);
      const expectedFee1 = amount1 * 10n / 10000n; // 0.1%
      expect(fee1).to.equal(expectedFee1);
      
      // Test minimum fee
      const amount2 = ethers.parseUnits("1", DECIMALS);
      const fee2 = await bridge.calculateRelayerFee(amount2);
      const minFee = await bridge.minRelayerFee();
      expect(fee2).to.equal(minFee); // Should use minimum
      
      console.log("✅ Fee calculation correct");
      console.log("   100 USDC -> fee:", ethers.formatUnits(fee1, DECIMALS), "USDC");
      console.log("   1 USDC -> fee:", ethers.formatUnits(fee2, DECIMALS), "USDC (minimum)");
    });
  });
});
