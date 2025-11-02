//! Guest program for verifying Solana bridge orders
//! This program verifies that a transfer order exists and is in the correct state

#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolType;
use fibonacci_lib::bridge::{
    BridgeProofPublicValues, MerkleProof, OrderStatus, TransferOrder, hash_order,
};

pub fn main() {
    // 1. 读取输入：订单数据
    let order: TransferOrder = sp1_zkvm::io::read();
    
    // 2. 读取输入：Merkle 证明（证明订单在状态树中）
    let merkle_proof: MerkleProof = sp1_zkvm::io::read();
    
    // 3. 验证订单状态必须是 Pending
    assert_eq!(
        order.status,
        OrderStatus::Pending,
        "Order must be in Pending status"
    );
    
    // 4. 计算订单哈希
    let order_hash = hash_order(&order);
    
    // 5. 验证订单哈希与 Merkle proof 的 leaf 匹配
    assert_eq!(
        order_hash, merkle_proof.leaf,
        "Order hash must match Merkle proof leaf"
    );
    
    // 6. 验证 Merkle 证明
    assert!(merkle_proof.verify(), "Merkle proof verification failed");
    
    // 7. 验证金额大于 0
    assert!(order.amount > 0, "Amount must be greater than 0");
    
    // 8. 准备公开输出
    let target_chain = if order.source_chain == 0 { 1 } else { 0 };
    
    // 将 u64 转换为 U256
    let amount_u256 = alloy_sol_types::private::U256::from(order.amount);
    
    let public_values = BridgeProofPublicValues {
        orderId: order.order_id,
        sourceChain: order.source_chain,
        targetChain: target_chain,
        token: order.token.into(),
        amount: amount_u256,
        recipient: order.recipient.into(),
        stateRoot: merkle_proof.root.into(),
        timestamp: order.created_at,
    };
    
    // 9. 提交公开值
    let bytes = BridgeProofPublicValues::abi_encode(&public_values);
    sp1_zkvm::io::commit_slice(&bytes);
    
    // 10. 同时输出订单哈希供调试
    sp1_zkvm::io::commit_slice(&order_hash);
}
