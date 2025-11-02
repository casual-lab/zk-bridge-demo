//! Host Program for generating ZK proofs of bridge order verification
//! 
//! This program:
//! 1. Creates a test transfer order
//! 2. Builds a Merkle proof for the order
//! 3. Executes the guest program in zkVM to verify the order
//! 4. Generates a ZK proof
//! 5. Verifies the proof

use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use fibonacci_lib::bridge::{TransferOrder, OrderStatus, MerkleProof, hash_order};
use sha2::{Sha256, Digest};
use alloy_sol_types::SolType;
use fibonacci_lib::bridge::BridgeProofPublicValues;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const BRIDGE_VERIFY_ELF: &[u8] = include_elf!("fibonacci-program");

fn main() {
    // Setup logging
    sp1_sdk::utils::setup_logger();

    println!("🌉 Bridge Order ZK Proof Generation\n");

    // 1. Create a test transfer order
    let order = TransferOrder {
        order_id: 1,
        user: [0x01; 32],
        source_chain: 0, // Solana
        token: [0x02; 32],
        amount: 1_000_000, // 1 USDC (6 decimals)
        recipient: [0x03; 32],
        relayer_fee: 10_000, // 0.01 USDC
        created_at: 1699000000,
        status: OrderStatus::Pending,
    };

    println!("📝 Test Order:");
    println!("  Order ID: {}", order.order_id);
    println!("  Source Chain: {} (Solana)", order.source_chain);
    println!("  Amount: {} (1 USDC)", order.amount);
    println!("  Status: {:?}", order.status);
    println!();

    // 2. Build Merkle proof
    let order_hash = hash_order(&order);
    println!("🔑 Order Hash: 0x{}", hex::encode(order_hash));

    // Create sibling hashes for a Merkle tree
    // In real scenario, these would come from the actual state tree
    let sibling1 = {
        let mut hasher = Sha256::new();
        hasher.update(b"sibling1");
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    };

    let sibling2 = {
        let mut hasher = Sha256::new();
        hasher.update(b"sibling2");
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    };

    // Helper function to hash a pair with proper sorting
    let hash_pair = |a: &[u8; 32], b: &[u8; 32]| -> [u8; 32] {
        let mut hasher = Sha256::new();
        if a <= b {
            hasher.update(a);
            hasher.update(b);
        } else {
            hasher.update(b);
            hasher.update(a);
        }
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    };

    // Calculate root: hash(hash(leaf, sibling1), sibling2)
    let level1 = hash_pair(&order_hash, &sibling1);
    let root = hash_pair(&level1, &sibling2);

    let merkle_proof = MerkleProof {
        leaf: order_hash,
        proof: vec![sibling1, sibling2],
        root,
    };

    println!("🌳 Merkle Root: 0x{}", hex::encode(merkle_proof.root));
    println!();

    // Verify the Merkle proof locally first
    assert!(merkle_proof.verify(), "Merkle proof should be valid");
    println!("✅ Merkle proof verified locally\n");

    // 3. Setup inputs for the zkVM
    let mut stdin = SP1Stdin::new();
    stdin.write(&order);
    stdin.write(&merkle_proof);

    println!("🚀 Executing guest program in zkVM...\n");

    // 4. Generate the proof
    let client = ProverClient::from_env();
    
    // First, execute without proving to see the output
    println!("📊 Executing (without proof)...");
    let (output, report) = client.execute(BRIDGE_VERIFY_ELF, &stdin).run().unwrap();
    println!("✅ Execution successful!");
    println!("   Cycles: {}", report.total_instruction_count());
    
    // Read the public values
    let public_values_bytes = output.as_slice();
    println!("   Public values length: {} bytes", public_values_bytes.len());
    
    // Decode public values (first 256 bytes are BridgeProofPublicValues)
    if public_values_bytes.len() >= 256 {
        let public_values = BridgeProofPublicValues::abi_decode(&public_values_bytes[0..256])
            .expect("Failed to decode public values");
        
        println!("\n📦 Decoded Public Values:");
        println!("   Order ID: {}", public_values.orderId);
        println!("   Source Chain: {}", public_values.sourceChain);
        println!("   Target Chain: {}", public_values.targetChain);
        println!("   Token: 0x{}", hex::encode(&public_values.token.0[28..]));
        println!("   Amount: {}", public_values.amount);
        println!("   Recipient: 0x{}", hex::encode(&public_values.recipient.0[28..]));
        println!("   State Root: 0x{}", hex::encode(&public_values.stateRoot.0));
        println!("   Timestamp: {}", public_values.timestamp);
    }
    
    // Read order hash (next 32 bytes)
    if public_values_bytes.len() >= 288 {
        let committed_order_hash = &public_values_bytes[256..288];
        println!("\n🔑 Committed Order Hash: 0x{}", hex::encode(committed_order_hash));
        assert_eq!(committed_order_hash, &order_hash, "Committed hash should match calculated hash");
    }
    
    println!("\n🔐 Generating ZK proof (this may take a while)...");
    
    // Generate the proof
    let (pk, vk) = client.setup(BRIDGE_VERIFY_ELF);
    let proof = client.prove(&pk, &stdin).run()
        .expect("Failed to generate proof");
    
    println!("✅ Proof generated successfully!");
    
    // 5. Verify the proof
    println!("\n🔍 Verifying proof...");
    client.verify(&proof, &vk).expect("Verification failed");
    
    println!("✅ Proof verified successfully!");
    
    println!("\n🎉 Bridge order verification complete!");
    println!("\n📊 Summary:");
    println!("   Order ID: {}", order.order_id);
    println!("   Amount: {} (1 USDC)", order.amount);
    println!("   Execution cycles: {}", report.total_instruction_count());
    println!("   Proof type: Core (for development)");
    println!();
    println!("💡 Note: To generate an on-chain verifiable proof, use:");
    println!("   - SP1ProofMode::Plonk for Plonk proofs");
    println!("   - SP1ProofMode::Groth16 for Groth16 proofs");
    
    // In a real implementation, you would:
    // - Save the proof to disk or database
    // - Submit the proof to the target chain's smart contract
    // - The contract would verify the proof and unlock tokens
}
