import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaEvmBridge } from "../target/types/solana_evm_bridge";
import { PublicKey, Keypair } from "@solana/web3.js";
import { 
  createMint, 
  createAccount, 
  mintTo, 
  getAccount,
} from "@solana/spl-token";
import { assert } from "chai";

describe("solana-evm-bridge with SPL Token", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SolanaEvmBridge as Program<SolanaEvmBridge>;
  
  const admin = provider.wallet as anchor.Wallet;
  const relayer = Keypair.generate();
  
  const EVM_CHAIN_ID = new anchor.BN(421614);
  
  let bridgeConfigPda: PublicKey;
  let tokenConfigPda: PublicKey;
  let vaultPda: PublicKey;
  
  let tokenMint: PublicKey;
  let userTokenAccount: PublicKey;
  let relayerTokenAccount: PublicKey;
  
  before(async () => {
    const signature = await provider.connection.requestAirdrop(
      relayer.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature);
  });
  
  it("Initialize bridge", async () => {
    [bridgeConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bridge_config")],
      program.programId
    );
    
    console.log("Bridge Config PDA:", bridgeConfigPda.toBase58());
    
    const tx = await program.methods
      .initializeBridge(EVM_CHAIN_ID)
      .accounts({
        bridgeConfig: bridgeConfigPda,
        admin: admin.publicKey,
      })
      .rpc();
    
    console.log("Initialize bridge tx:", tx);
    
    const bridgeConfig = await program.account.bridgeConfig.fetch(bridgeConfigPda);
    
    assert.equal(bridgeConfig.admin.toBase58(), admin.publicKey.toBase58());
    assert.equal(bridgeConfig.evmChainId.toString(), EVM_CHAIN_ID.toString());
    assert.equal(bridgeConfig.relayerFeeBps, 10);
    assert.equal(bridgeConfig.minRelayerFee.toString(), "50000");
    assert.equal(bridgeConfig.paused, false);
    
    console.log("✅ Bridge initialized successfully");
  });
  
  it("Create SPL Token", async () => {
    tokenMint = await createMint(
      provider.connection,
      admin.payer,
      admin.publicKey,
      null,
      6
    );
    
    console.log("Token Mint:", tokenMint.toBase58());
    
    userTokenAccount = await createAccount(
      provider.connection,
      admin.payer,
      tokenMint,
      admin.publicKey
    );
    
    console.log("User Token Account:", userTokenAccount.toBase58());
    
    relayerTokenAccount = await createAccount(
      provider.connection,
      admin.payer,
      tokenMint,
      relayer.publicKey
    );
    
    console.log("Relayer Token Account:", relayerTokenAccount.toBase58());
    
    await mintTo(
      provider.connection,
      admin.payer,
      tokenMint,
      userTokenAccount,
      admin.publicKey,
      100_000_000
    );
    
    console.log("✅ SPL Token created and minted");
  });
  
  it("Register token pair", async () => {
    [tokenConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_config"), tokenMint.toBuffer()],
      program.programId
    );
    
    console.log("Token Config PDA:", tokenConfigPda.toBase58());
    
    const evmToken = Array(20).fill(1);
    
    const tx = await program.methods
      .registerTokenPair(evmToken, true)
      .accounts({
        tokenConfig: tokenConfigPda,
        solanaMint: tokenMint,
        admin: admin.publicKey,
      })
      .rpc();
    
    console.log("Register token pair tx:", tx);
    
    const tokenConfig = await program.account.tokenConfig.fetch(tokenConfigPda);
    assert.equal(tokenConfig.solanaMint.toBase58(), tokenMint.toBase58());
    assert.deepEqual(Array.from(tokenConfig.evmToken), evmToken);
    assert.equal(tokenConfig.isNativeSolana, true);
    assert.equal(tokenConfig.totalLocked.toString(), "0");
    
    console.log("✅ Token pair registered successfully");
  });
  
  it("Initialize vault", async () => {
    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), tokenMint.toBuffer()],
      program.programId
    );
    
    console.log("Vault PDA:", vaultPda.toBase58());
    
    const tx = await program.methods
      .initializeVault()
      .accounts({
        vault: vaultPda,
        solanaMint: tokenMint,
        admin: admin.publicKey,
      })
      .rpc();
    
    console.log("Initialize vault tx:", tx);
    
    const vaultAccount = await getAccount(provider.connection, vaultPda);
    assert.equal(vaultAccount.mint.toBase58(), tokenMint.toBase58());
    assert.equal(vaultAccount.amount.toString(), "0");
    
    console.log("✅ Vault initialized successfully");
  });
  
  it("Lock tokens", async () => {
    const orderId = 1;
    const amount = 1_000_000;
    const recipientEvm = Array(20).fill(2);
    
    const [orderPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("transfer_order"), new anchor.BN(orderId).toArrayLike(Buffer, "le", 8)],
      program.programId
    );
    
    console.log("Order PDA:", orderPda.toBase58());
    console.log("Order ID:", orderId);
    
    const userAccountBefore = await getAccount(provider.connection, userTokenAccount);
    console.log("User balance before:", userAccountBefore.amount.toString());
    
    const tx = await program.methods
      .lockTokens(new anchor.BN(amount), recipientEvm)
      .accounts({
        order: orderPda,
        bridgeConfig: bridgeConfigPda,
        tokenConfig: tokenConfigPda,
        user: admin.publicKey,
        userTokenAccount,
        vault: vaultPda,
      })
      .rpc();
    
    console.log("Lock tokens tx:", tx);
    
    const order = await program.account.transferOrder.fetch(orderPda);
    assert.equal(order.orderId.toString(), orderId.toString());
    assert.equal(order.user.toBase58(), admin.publicKey.toBase58());
    assert.ok("pending" in order.status);
    
    const relayerFee = Math.floor(amount * 10 / 10000);
    const amountLocked = amount - relayerFee;
    
    assert.equal(order.amount.toString(), amountLocked.toString());
    assert.equal(order.relayerFee.toString(), relayerFee.toString());
    
    const vaultAccountAfter = await getAccount(provider.connection, vaultPda);
    assert.equal(vaultAccountAfter.amount.toString(), amountLocked.toString());
    
    const tokenConfig = await program.account.tokenConfig.fetch(tokenConfigPda);
    assert.equal(tokenConfig.totalLocked.toString(), amountLocked.toString());
    
    console.log("✅ Tokens locked successfully");
    console.log("   Order ID:", orderId);
    console.log("   Amount locked:", amountLocked);
    console.log("   Relayer Fee:", relayerFee);
    console.log("   Vault balance:", vaultAccountAfter.amount.toString());
  });
  
  it("Unlock tokens", async () => {
    const orderId = 1;
    const mockProofHash = Array(32).fill(1);
    
    const [orderPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("transfer_order"), new anchor.BN(orderId).toArrayLike(Buffer, "le", 8)],
      program.programId
    );
    
    console.log("Unlocking Order PDA:", orderPda.toBase58());
    console.log("Unlocking Order ID:", orderId);
    
    const userAccountBefore = await getAccount(provider.connection, userTokenAccount);
    const relayerAccountBefore = await getAccount(provider.connection, relayerTokenAccount);
    const vaultAccountBefore = await getAccount(provider.connection, vaultPda);
    const orderBefore = await program.account.transferOrder.fetch(orderPda);
    
    console.log("User balance before unlock:", userAccountBefore.amount.toString());
    console.log("Vault balance before unlock:", vaultAccountBefore.amount.toString());
    console.log("Order status before:", orderBefore.status);
    
    const tx = await program.methods
      .unlockTokens(new anchor.BN(orderId), mockProofHash)
      .accounts({
        order: orderPda,
        bridgeConfig: bridgeConfigPda,
        tokenConfig: tokenConfigPda,
        userTokenAccount,
        vault: vaultPda,
        relayer: relayer.publicKey,
        relayerRewardAccount: relayerTokenAccount,
      })
      .signers([relayer])
      .rpc();
    
    console.log("Unlock tokens tx:", tx);
    
    const orderAfter = await program.account.transferOrder.fetch(orderPda);
    
    assert.ok("completed" in orderAfter.status);
    assert.deepEqual(Array.from(orderAfter.proofHash), mockProofHash);
    assert.equal(orderAfter.completedBy.toBase58(), relayer.publicKey.toBase58());
    assert.ok(orderAfter.completedAt.toNumber() > 0);
    
    const userAccountAfter = await getAccount(provider.connection, userTokenAccount);
    const relayerAccountAfter = await getAccount(provider.connection, relayerTokenAccount);
    const vaultAccountAfter = await getAccount(provider.connection, vaultPda);
    
    const totalAmount = Number(orderBefore.amount);
    const relayerFee = Math.max(
      Math.floor(totalAmount * 10 / 10000),
      50000
    );
    const userAmount = totalAmount - relayerFee;
    
    assert.equal(
      Number(userAccountAfter.amount - userAccountBefore.amount),
      userAmount
    );
    
    assert.equal(
      Number(relayerAccountAfter.amount - relayerAccountBefore.amount),
      relayerFee
    );
    
    assert.equal(
      Number(vaultAccountBefore.amount - vaultAccountAfter.amount),
      totalAmount
    );
    
    const tokenConfig = await program.account.tokenConfig.fetch(tokenConfigPda);
    assert.equal(tokenConfig.totalLocked.toString(), "0");
    
    console.log("✅ Tokens unlocked successfully");
    console.log("   User received:", userAmount);
    console.log("   Relayer fee:", relayerFee);
    console.log("   Order status:", orderAfter.status);
    console.log("   User balance after:", userAccountAfter.amount.toString());
    console.log("   Vault balance after:", vaultAccountAfter.amount.toString());
    console.log("   Total locked:", tokenConfig.totalLocked.toString());
  });
});
