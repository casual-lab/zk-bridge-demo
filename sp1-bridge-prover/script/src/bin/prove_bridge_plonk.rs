//! Host Program for generating on-chain verifiable ZK proofs (Plonk)
//! 
//! This program generates a Plonk proof that can be verified on-chain.

use sp1_sdk::{include_elf, ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use fibonacci_lib::bridge::{TransferOrder, OrderStatus, MerkleProof, hash_order};
use sha2::{Sha256, Digest};
use alloy_sol_types::SolType;
use fibonacci_lib::bridge::BridgeProofPublicValues;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const BRIDGE_VERIFY_ELF: &[u8] = include_elf!("fibonacci-program");

fn main() {
    // Setup logging
    sp1_sdk::utils::setup_logger();

    println!("🌉 Bridge Order ZK Proof Generation (Plonk Mode)\n");

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

    // Calculate root
    let level1 = hash_pair(&order_hash, &sibling1);
    let root = hash_pair(&level1, &sibling2);

    let merkle_proof = MerkleProof {
        leaf: order_hash,
        proof: vec![sibling1, sibling2],
        root,
    };

    println!("🌳 Merkle Root: 0x{}", hex::encode(merkle_proof.root));
    println!();

    assert!(merkle_proof.verify(), "Merkle proof should be valid");
    println!("✅ Merkle proof verified locally\n");

    // 3. Setup inputs for the zkVM
    let mut stdin = SP1Stdin::new();
    stdin.write(&order);
    stdin.write(&merkle_proof);

    println!("🚀 Generating Plonk proof (this will take several minutes)...\n");

    // 4. Generate the Plonk proof
    let client = ProverClient::from_env();
    
    let (pk, vk) = client.setup(BRIDGE_VERIFY_ELF);
    
    // Generate Plonk proof for on-chain verification
    let proof: SP1ProofWithPublicValues = client
        .prove(&pk, &stdin)
        .plonk()
        .run()
        .expect("Failed to generate Plonk proof");
    
    println!("✅ Plonk proof generated successfully!");
    
    // 5. Verify the proof
    println!("\n🔍 Verifying Plonk proof...");
    client.verify(&proof, &vk).expect("Verification failed");
    
    println!("✅ Plonk proof verified successfully!");
    
    // 6. Extract public values
    let public_values_bytes = proof.public_values.as_slice();
    
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
    
    println!("\n🎉 On-chain verifiable proof generation complete!");
    println!("\n📊 Summary:");
    println!("   Order ID: {}", order.order_id);
    println!("   Amount: {} (1 USDC)", order.amount);
    println!("   Proof type: Plonk (on-chain verifiable)");
    println!("   Proof size: {} bytes", proof.bytes().len());
    
    println!("\n💾 Next steps:");
    println!("   1. Deploy SP1 Plonk Verifier contract to EVM");
    println!("   2. Submit this proof to the verifier contract");
    println!("   3. If verification passes, unlock tokens on target chain");
}
