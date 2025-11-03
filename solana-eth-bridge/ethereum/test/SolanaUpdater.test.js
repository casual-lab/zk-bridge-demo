import { expect } from "chai";
import hre from "hardhat";
const { ethers } = hre;

describe("SolanaUpdater", function () {
  let solanaUpdater;
  let owner;
  let otherAccount;

  beforeEach(async function () {
    [owner, otherAccount] = await ethers.getSigners();
    
    const SolanaUpdater = await ethers.getContractFactory("SolanaUpdater");
    solanaUpdater = await SolanaUpdater.deploy(
      ethers.ZeroAddress // SP1 Verifier 占位
    );
  });

  describe("初始化", function () {
    it("应该正确设置 admin", async function () {
      expect(await solanaUpdater.admin()).to.equal(owner.address);
    });

    it("应该正确设置 sp1Verifier", async function () {
      expect(await solanaUpdater.sp1Verifier()).to.equal(ethers.ZeroAddress);
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
      
      const proof = "0x"; // 空证明（测试时跳过验证）
      
      await expect(solanaUpdater.updateSolanaBlock(proof, header))
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
      
      await expect(
        solanaUpdater.updateSolanaBlock("0x", header)
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
      
      await solanaUpdater.updateSolanaBlock("0x", header1);
      
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
        solanaUpdater.updateSolanaBlock("0x", header2)
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
      
      await solanaUpdater.updateSolanaBlock("0x", header1);
      
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
        solanaUpdater.updateSolanaBlock("0x", header2)
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
      
      await solanaUpdater.updateSolanaBlock("0x", header1);
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
      
      await solanaUpdater.updateSolanaBlock("0x", header2);
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
      
      await solanaUpdater.updateSolanaBlock("0x", header3);
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
      
      await solanaUpdater.updateSolanaBlock("0x", header);
      
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
      
      // 为每个区块生成空证明
      const proofs = headers.map(() => "0x");
      
      await solanaUpdater.updateBatchSolanaBlocks(proofs, headers);
      
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
