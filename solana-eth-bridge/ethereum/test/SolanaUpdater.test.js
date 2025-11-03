import { expect } from "chai";
import hre from "hardhat";
const { ethers } = hre;

describe("SolanaUpdater", function () {
  let solanaUpdater;
  let mockVerifier;
  let owner;
  let otherAccount;

  beforeEach(async function () {
    [owner, otherAccount] = await ethers.getSigners();
    
    // 部署 MockSP1Verifier (strictMode = false for easier testing)
    const MockSP1Verifier = await ethers.getContractFactory("MockSP1Verifier");
    mockVerifier = await MockSP1Verifier.deploy(false);
    
    // 部署 SolanaUpdater 并传入 verifier 地址
    const SolanaUpdater = await ethers.getContractFactory("SolanaUpdater");
    solanaUpdater = await SolanaUpdater.deploy(
      await mockVerifier.getAddress()
    );
  });

  describe("初始化", function () {
    it("应该正确设置 admin", async function () {
      expect(await solanaUpdater.admin()).to.equal(owner.address);
    });

    it("应该正确设置 sp1Verifier", async function () {
      expect(await solanaUpdater.sp1Verifier()).to.equal(await mockVerifier.getAddress());
    });

    it("初始 currentSlot 应该为 0", async function () {
      expect(await solanaUpdater.currentSlot()).to.equal(0);
    });
  });

  describe("updateSolanaBlock", function () {
    it("应该能更新第一个 Solana 区块", async function () {
      const header = {
        slot: 1000,
        blockhash: ethers.randomBytes(32),
        parentHash: ethers.ZeroHash,
        blockHeight: 1000,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      const programVKey = ethers.randomBytes(32);
      const publicValues = ethers.hexlify(ethers.randomBytes(64)); // 模拟公开输入
      const proof = ethers.hexlify(ethers.randomBytes(256)); // 模拟证明
      
      await expect(solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header))
        .to.emit(solanaUpdater, "SolanaBlockUpdated")
        .withArgs(header.slot, header.blockhash, header.blockHeight);
      
      expect(await solanaUpdater.currentSlot()).to.equal(1000);
    });

    it("应该拒绝确认数不足的区块 (< 32)", async function () {
      const header = {
        slot: 1000,
        blockhash: ethers.randomBytes(32),
        parentHash: ethers.ZeroHash,
        blockHeight: 1000,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 10 // 不足 32
      };
      
      const programVKey = ethers.randomBytes(32);
      const publicValues = ethers.hexlify(ethers.randomBytes(64));
      const proof = ethers.hexlify(ethers.randomBytes(256));
      
      await expect(
        solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header)
      ).to.be.revertedWith("Insufficient confirmations");
    });

    it("应该验证区块连续性 - parentHash 必须匹配", async function () {
      // 先添加第一个区块
      const header1 = {
        slot: 1000,
        blockhash: ethers.hexlify(ethers.randomBytes(32)),
        parentHash: ethers.ZeroHash,
        blockHeight: 1000,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      const programVKey = ethers.randomBytes(32);
      const publicValues = ethers.hexlify(ethers.randomBytes(64));
      const proof = ethers.hexlify(ethers.randomBytes(256));
      
      await solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header1);
      
      // 尝试添加 parentHash 不匹配的区块
      const header2 = {
        slot: 1001,
        blockhash: ethers.randomBytes(32),
        parentHash: ethers.randomBytes(32), // 错误的 parent hash
        blockHeight: 1001,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      await expect(
        solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header2)
      ).to.be.revertedWith("Parent hash mismatch");
    });

    it("应该验证区块连续性 - slot 必须递增", async function () {
      const header1 = {
        slot: 1000,
        blockhash: ethers.hexlify(ethers.randomBytes(32)),
        parentHash: ethers.ZeroHash,
        blockHeight: 1000,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      const programVKey = ethers.randomBytes(32);
      const publicValues = ethers.hexlify(ethers.randomBytes(64));
      const proof = ethers.hexlify(ethers.randomBytes(256));
      
      await solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header1);
      
      // 尝试添加 slot 相同的区块
      const header2 = {
        slot: 1000, // 相同的 slot
        blockhash: ethers.randomBytes(32),
        parentHash: header1.blockhash,
        blockHeight: 1000,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      await expect(
        solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header2)
      ).to.be.revertedWith("Slot must be greater than current");
    });

    it("应该能连续添加多个区块", async function () {
      // 区块 1
      const header1 = {
        slot: 1000,
        blockhash: ethers.hexlify(ethers.randomBytes(32)),
        parentHash: ethers.ZeroHash,
        blockHeight: 1000,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      const programVKey = ethers.randomBytes(32);
      const publicValues = ethers.hexlify(ethers.randomBytes(64));
      const proof = ethers.hexlify(ethers.randomBytes(256));
      
      await solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header1);
      expect(await solanaUpdater.currentSlot()).to.equal(1000);
      
      // 区块 2
      const header2 = {
        slot: 1001,
        blockhash: ethers.hexlify(ethers.randomBytes(32)),
        parentHash: header1.blockhash,
        blockHeight: 1001,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      await solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header2);
      expect(await solanaUpdater.currentSlot()).to.equal(1001);
      
      // 区块 3
      const header3 = {
        slot: 1002,
        blockhash: ethers.hexlify(ethers.randomBytes(32)),
        parentHash: header2.blockhash,
        blockHeight: 1002,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      await solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header3);
      expect(await solanaUpdater.currentSlot()).to.equal(1002);
    });
  });

  describe("getSolanaBlock", function () {
    it("应该能查询存储的区块", async function () {
      const header = {
        slot: 1000,
        blockhash: ethers.hexlify(ethers.randomBytes(32)),
        parentHash: ethers.ZeroHash,
        blockHeight: 1000,
        timestamp: Math.floor(Date.now() / 1000),
        confirmations: 32
      };
      
      const programVKey = ethers.randomBytes(32);
      const publicValues = ethers.hexlify(ethers.randomBytes(64));
      const proof = ethers.hexlify(ethers.randomBytes(256));
      
      await solanaUpdater.updateSolanaBlock(programVKey, publicValues, proof, header);
      
      const stored = await solanaUpdater.getSolanaBlock(1000);
      
      expect(stored.slot).to.equal(header.slot);
      expect(stored.blockhash).to.equal(header.blockhash);
      expect(stored.parentHash).to.equal(header.parentHash);
      expect(stored.blockHeight).to.equal(header.blockHeight);
      expect(stored.confirmations).to.equal(header.confirmations);
    });

    it("查询不存在的区块应该返回空数据", async function () {
      const stored = await solanaUpdater.getSolanaBlock(9999);
      
      expect(stored.slot).to.equal(0);
      expect(stored.blockHeight).to.equal(0);
    });
  });

  describe("updateBatchSolanaBlocks", function () {
    it("应该能批量更新多个区块", async function () {
      const headers = [
        {
          slot: 1000,
          blockhash: ethers.hexlify(ethers.randomBytes(32)),
          parentHash: ethers.ZeroHash,
          blockHeight: 1000,
          timestamp: Math.floor(Date.now() / 1000),
          confirmations: 32
        }
      ];
      
      // 生成后续区块
      for (let i = 1; i < 5; i++) {
        headers.push({
          slot: 1000 + i,
          blockhash: ethers.hexlify(ethers.randomBytes(32)),
          parentHash: headers[i - 1].blockhash,
          blockHeight: 1000 + i,
          timestamp: Math.floor(Date.now() / 1000),
          confirmations: 32
        });
      }
      
      const programVKey = ethers.randomBytes(32);
      const publicValuesList = headers.map(() => ethers.hexlify(ethers.randomBytes(64)));
      const proofs = headers.map(() => ethers.hexlify(ethers.randomBytes(256)));
      
      await solanaUpdater.updateBatchSolanaBlocks(programVKey, publicValuesList, proofs, headers);
      
      expect(await solanaUpdater.currentSlot()).to.equal(1004);
      
      // 验证所有区块都存储了
      for (let i = 0; i < headers.length; i++) {
        const stored = await solanaUpdater.getSolanaBlock(headers[i].slot);
        expect(stored.slot).to.equal(headers[i].slot);
      }
    });
  });

  describe("权限管理", function () {
    it("应该能更改 admin", async function () {
      await expect(solanaUpdater.setAdmin(otherAccount.address))
        .to.emit(solanaUpdater, "AdminChanged")
        .withArgs(owner.address, otherAccount.address);
      
      expect(await solanaUpdater.admin()).to.equal(otherAccount.address);
    });

    it("只有 admin 能更改 admin", async function () {
      await expect(
        solanaUpdater.connect(otherAccount).setAdmin(otherAccount.address)
      ).to.be.revertedWith("Only admin");
    });
  });
});
