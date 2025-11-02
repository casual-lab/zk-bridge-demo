//! Test script for bridge order verification
//! 
//! This script demonstrates verifying Solana bridge orders using SP1 zkVM

use clap::Parser;
use fibonacci_lib::bridge::{
    BridgeProofPublicValues, MerkleProof, OrderStatus, TransferOrder, hash_order,
};
use alloy_sol_types::SolType;
use sp1_sdk::{ProverClient, SP1Stdin};

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about = "Bridge order verification with SP1", long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    #[arg(long, default_value = "1")]
    order_id: u64,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::from_env();

    // 创建测试订单
    let order = TransferOrder {
        order_id: args.order_id,
        user: [0x01; 32],           // 模拟 Solana 地址
        source_chain: 0,             // Solana
        token: [0x02; 32],          // USDC 代币地址
        amount: 1_000_000,          // 1 USDC (6 decimals)
        recipient: [0x03; 32],      // EVM 接收地址
        relayer_fee: 1_000,         // 0.001 USDC
        created_at: 100,            // slot number
        status: OrderStatus::Pending,
    };

    println!("🌉 Testing Bridge Order Verification");
    println!("====================================");
    println!("Order ID: {}", order.order_id);
    println!("Source Chain: {} (Solana)", order.source_chain);
    println!("Amount: {} lamports", order.amount);
    println!("Status: {:?}", order.status);
    println!();

    // 计算订单哈希
    let order_hash = hash_order(&order);
    println!("Order Hash: 0x{}", hex::encode(order_hash));

    // 创建简单的 Merkle proof (单节点树)
    let sibling = [0x00; 32]; // 简化版：假设只有一个订单
    let root = if order_hash <= sibling {
        hash_pair(&order_hash, &sibling)
    } else {
        hash_pair(&sibling, &order_hash)
    };

    let merkle_proof = MerkleProof {
        leaf: order_hash,
        proof: vec![sibling],
        root,
    };

    println!("Merkle Root: 0x{}", hex::encode(root));
    println!();

    // 注意：这里需要先编译 bridge_verify 程序
    // 由于我们还没有编译它，暂时使用 fibonacci 程序作为示例
    println!("⚠️  Note: Bridge verification program not yet compiled.");
    println!("This is a demonstration of the data structures.");
    println!();

    // 准备输入
    let mut stdin = SP1Stdin::new();
    stdin.write(&order);
    stdin.write(&merkle_proof);

    // 验证 Merkle proof
    if merkle_proof.verify() {
        println!("✅ Merkle proof verified successfully!");
    } else {
        println!("❌ Merkle proof verification failed!");
        return;
    }

    // 输出期望的公开值
    let target_chain = if order.source_chain == 0 { 1 } else { 0 };
    let amount_u256 = alloy_sol_types::private::U256::from(order.amount);
    
    let expected_public_values = BridgeProofPublicValues {
        orderId: order.order_id,
        sourceChain: order.source_chain,
        targetChain: target_chain,
        token: order.token.into(),
        amount: amount_u256,
        recipient: order.recipient.into(),
        stateRoot: merkle_proof.root.into(),
        timestamp: order.created_at,
    };

    println!("\n📊 Expected Public Values:");
    println!("====================================");
    println!("Order ID: {}", expected_public_values.orderId);
    println!("Source Chain: {}", expected_public_values.sourceChain);
    println!("Target Chain: {}", expected_public_values.targetChain);
    println!("Token: 0x{}", hex::encode(expected_public_values.token));
    println!("Amount: {}", expected_public_values.amount);
    println!("Recipient: 0x{}", hex::encode(expected_public_values.recipient));
    println!("State Root: 0x{}", hex::encode(expected_public_values.stateRoot));
    println!("Timestamp: {}", expected_public_values.timestamp);

    let bytes = BridgeProofPublicValues::abi_encode(&expected_public_values);
    println!("\n📦 ABI Encoded Public Values ({} bytes)", bytes.len());
    println!("0x{}", hex::encode(&bytes[..64.min(bytes.len())]));
    if bytes.len() > 64 {
        println!("... ({} more bytes)", bytes.len() - 64);
    }

    println!("\n✅ Bridge order verification test completed!");
    println!("Next steps:");
    println!("  1. Compile the bridge_verify guest program");
    println!("  2. Run with --execute to test execution");
    println!("  3. Run with --prove to generate ZK proof");
}

/// Helper function to hash两个值
fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(a);
    hasher.update(b);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}
