import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaBridge } from "../target/types/solana_bridge";
import { expect } from "chai";

describe("solana-bridge", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SolanaBridge as Program<SolanaBridge>;
  
  let bridgeState: anchor.web3.PublicKey;
  let admin: anchor.web3.Keypair;

  before(async () => {
    admin = (provider.wallet as anchor.Wallet).payer;
    
    [bridgeState] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("bridge_state")],
      program.programId
    );
  });

  describe("初始化", () => {
    it("应该正确初始化桥接程序", async () => {
      await program.methods
        .initialize(admin.publicKey)
        .accounts({
          bridgeState,
          payer: admin.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      const state = await program.account.bridgeState.fetch(bridgeState);
      expect(state.admin.toString()).to.equal(admin.publicKey.toString());
      expect(state.lastEthBlock.toNumber()).to.equal(0);
      expect(state.ethHeaders).to.have.length(0);
    });

    it("初始化后应该能查询状态", async () => {
      const state = await program.account.bridgeState.fetch(bridgeState);
      expect(state.admin).to.not.be.undefined;
      expect(state.ethHeaders).to.be.an('array');
    });
  });

  describe("验证以太坊区块", () => {
    it("应该能验证第一个以太坊区块", async () => {
      const header = {
        blockNumber: new anchor.BN(1000),
        blockHash: Buffer.alloc(32, 1),
        parentHash: Buffer.alloc(32, 0),
        timestamp: new anchor.BN(Math.floor(Date.now() / 1000)),
        stateRoot: Buffer.alloc(32, 2),
        transactionsRoot: Buffer.alloc(32, 3),
        receiptsRoot: Buffer.alloc(32, 4),
        confirmations: 32,
      };

      await program.methods
        .verifyEthBlock(Buffer.alloc(0), header)
        .accounts({
          bridgeState,
          authority: admin.publicKey,
        })
        .rpc();

      const state = await program.account.bridgeState.fetch(bridgeState);
      expect(state.lastEthBlock.toNumber()).to.equal(1000);
      expect(state.ethHeaders).to.have.length(1);
      expect(state.ethHeaders[0].blockNumber.toNumber()).to.equal(1000);
    });

    it("应该拒绝确认数不足的区块 (< 12)", async () => {
      const state = await program.account.bridgeState.fetch(bridgeState);
      const lastHeader = state.ethHeaders[state.ethHeaders.length - 1];

      const header = {
        blockNumber: new anchor.BN(1001),
        blockHash: Buffer.alloc(32, 2),
        parentHash: Buffer.from(lastHeader.blockHash),
        timestamp: new anchor.BN(Math.floor(Date.now() / 1000)),
        stateRoot: Buffer.alloc(32, 2),
        transactionsRoot: Buffer.alloc(32, 3),
        receiptsRoot: Buffer.alloc(32, 4),
        confirmations: 5, // 不足 12
      };

      try {
        await program.methods
          .verifyEthBlock(Buffer.alloc(0), header)
          .accounts({
            bridgeState,
            authority: admin.publicKey,
          })
          .rpc();
        
        expect.fail("应该抛出 InsufficientConfirmations 错误");
      } catch (error) {
        expect(error.toString()).to.include("InsufficientConfirmations");
      }
    });

    it("应该验证区块连续性 - parentHash 必须匹配", async () => {
      const state = await program.account.bridgeState.fetch(bridgeState);
      const lastHeader = state.ethHeaders[state.ethHeaders.length - 1];

      // 正确的 header（parentHash 匹配）
      const correctHeader = {
        blockNumber: new anchor.BN(1001),
        blockHash: Buffer.alloc(32, 2),
        parentHash: Buffer.from(lastHeader.blockHash), // 使用上一个区块的 hash
        timestamp: new anchor.BN(Math.floor(Date.now() / 1000)),
        stateRoot: Buffer.alloc(32, 2),
        transactionsRoot: Buffer.alloc(32, 3),
        receiptsRoot: Buffer.alloc(32, 4),
        confirmations: 32,
      };

      await program.methods
        .verifyEthBlock(Buffer.alloc(0), correctHeader)
        .accounts({
          bridgeState,
          authority: admin.publicKey,
        })
        .rpc();

      const newState = await program.account.bridgeState.fetch(bridgeState);
      expect(newState.lastEthBlock.toNumber()).to.equal(1001);
    });

    it("应该拒绝 parentHash 不匹配的区块", async () => {
      const wrongHeader = {
        blockNumber: new anchor.BN(1002),
        blockHash: Buffer.alloc(32, 3),
        parentHash: Buffer.alloc(32, 99), // 错误的 parent hash
        timestamp: new anchor.BN(Math.floor(Date.now() / 1000)),
        stateRoot: Buffer.alloc(32, 2),
        transactionsRoot: Buffer.alloc(32, 3),
        receiptsRoot: Buffer.alloc(32, 4),
        confirmations: 32,
      };

      try {
        await program.methods
          .verifyEthBlock(Buffer.alloc(0), wrongHeader)
          .accounts({
            bridgeState,
            authority: admin.publicKey,
          })
          .rpc();
        
        expect.fail("应该抛出 ParentHashMismatch 错误");
      } catch (error) {
        expect(error.toString()).to.include("ParentHashMismatch");
      }
    });

    it("应该能连续添加多个区块", async () => {
      const state = await program.account.bridgeState.fetch(bridgeState);
      let prevHash = Buffer.from(state.ethHeaders[state.ethHeaders.length - 1].blockHash);
      let prevNumber = state.ethHeaders[state.ethHeaders.length - 1].blockNumber.toNumber();

      // 添加 3 个连续区块
      for (let i = 1; i <= 3; i++) {
        const header = {
          blockNumber: new anchor.BN(prevNumber + 1),
          blockHash: Buffer.alloc(32, prevNumber + 1),
          parentHash: prevHash,
          timestamp: new anchor.BN(Math.floor(Date.now() / 1000)),
          stateRoot: Buffer.alloc(32, 2),
          transactionsRoot: Buffer.alloc(32, 3),
          receiptsRoot: Buffer.alloc(32, 4),
          confirmations: 32,
        };

        await program.methods
          .verifyEthBlock(Buffer.alloc(0), header)
          .accounts({
            bridgeState,
            authority: admin.publicKey,
          })
          .rpc();

        prevHash = header.blockHash;
        prevNumber = header.blockNumber.toNumber();
      }

      const finalState = await program.account.bridgeState.fetch(bridgeState);
      expect(finalState.lastEthBlock.toNumber()).to.equal(prevNumber);
      expect(finalState.ethHeaders.length).to.be.greaterThan(3);
    });
  });

  describe("执行跨链消息", () => {
    it("应该能执行基于已验证区块的消息", async () => {
      const messageHash = Buffer.alloc(32, 5);
      const merkleProof = [
        Buffer.alloc(32, 6),
        Buffer.alloc(32, 7),
      ];

      await program.methods
        .executeMessage(
          new anchor.BN(1000),
          messageHash,
          merkleProof
        )
        .accounts({
          bridgeState,
          authority: admin.publicKey,
        })
        .rpc();

      // 如果没有抛出错误，说明执行成功
    });

    it("查询不存在的区块应该失败", async () => {
      const messageHash = Buffer.alloc(32, 5);
      const merkleProof = [];

      try {
        await program.methods
          .executeMessage(
            new anchor.BN(9999), // 不存在的区块
            messageHash,
            merkleProof
          )
          .accounts({
            bridgeState,
            authority: admin.publicKey,
          })
          .rpc();
        
        expect.fail("应该抛出 BlockNotFound 错误");
      } catch (error) {
        expect(error.toString()).to.include("BlockNotFound");
      }
    });
  });

  describe("查询功能", () => {
    it("应该能查询存储的所有区块头", async () => {
      const state = await program.account.bridgeState.fetch(bridgeState);
      
      expect(state.ethHeaders).to.be.an('array');
      expect(state.ethHeaders.length).to.be.greaterThan(0);
      
      // 验证第一个区块
      const firstHeader = state.ethHeaders[0];
      expect(firstHeader.blockNumber.toNumber()).to.equal(1000);
      expect(firstHeader.confirmations).to.equal(32);
    });

    it("应该能查询当前状态", async () => {
      const state = await program.account.bridgeState.fetch(bridgeState);
      
      expect(state.admin.toString()).to.equal(admin.publicKey.toString());
      expect(state.lastEthBlock.toNumber()).to.be.greaterThan(0);
      
      console.log(`  当前状态:`);
      console.log(`    Admin: ${state.admin.toString()}`);
      console.log(`    Last ETH Block: ${state.lastEthBlock.toNumber()}`);
      console.log(`    Total Headers: ${state.ethHeaders.length}`);
    });
  });
});
