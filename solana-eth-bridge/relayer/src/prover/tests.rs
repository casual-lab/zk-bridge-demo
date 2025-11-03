use crate::prover::{Sp1Prover, types::*};

#[test]
fn test_prover_creation() {
    // 测试创建 Prover (不实际生成证明,太慢)
    let prover = Sp1Prover::new();
    println!("✅ Prover created successfully");
}

#[test]
fn test_mock_solana_block_data() {
    // 创建测试用的 Solana 区块数据
    let block_data = SolanaBlockData {
        slot: 12345,
        parent_slot: 12344,
        blockhash: [1u8; 32],
        parent_hash: [2u8; 32],
        signatures: vec![
            ValidatorSignature {
                validator_pubkey: [3u8; 32],
                signature: vec![4u8; 64],
            },
        ],
    };
    
    // 测试序列化
    let encoded = bincode::serialize(&block_data).unwrap();
    println!("✅ Serialized Solana block data: {} bytes", encoded.len());
    
    // 测试反序列化
    let decoded: SolanaBlockData = bincode::deserialize(&encoded).unwrap();
    assert_eq!(decoded.slot, 12345);
    println!("✅ Deserialized successfully");
}

#[test]
fn test_mock_ethereum_block_data() {
    // 创建测试用的 Ethereum 区块数据
    let block_data = EthereumBlockData {
        number: 100,
        hash: [5u8; 32],
        parent_hash: [6u8; 32],
        timestamp: 1699000000,
        state_root: [7u8; 32],
    };
    
    // 测试序列化
    let encoded = bincode::serialize(&block_data).unwrap();
    println!("✅ Serialized Ethereum block data: {} bytes", encoded.len());
    
    // 测试反序列化
    let decoded: EthereumBlockData = bincode::deserialize(&encoded).unwrap();
    assert_eq!(decoded.number, 100);
    println!("✅ Deserialized successfully");
}

// 注意: 实际的证明生成测试会非常慢 (10-30秒),
// 在生产环境中应该使用集成测试而不是单元测试
#[test]
#[ignore] // 使用 --ignored 标志来运行
fn test_prove_solana_block_real() {
    let prover = Sp1Prover::new();
    
    let block_data = SolanaBlockData {
        slot: 1,
        parent_slot: 0,
        blockhash: [1u8; 32],
        parent_hash: [0u8; 32],
        signatures: vec![
            ValidatorSignature {
                validator_pubkey: [1u8; 32],
                signature: vec![1u8; 64],
            },
        ],
    };
    
    println!("🔬 Starting proof generation (this may take 10-30 seconds)...");
    let result = prover.prove_solana_block(&block_data);
    
    match result {
        Ok(proof) => {
            println!("✅ Proof generated successfully!");
            println!("   Proof size: {} bytes", proof.proof_bytes.len());
            println!("   Public values: {} bytes", proof.public_values.len());
            assert!(proof.proof_bytes.len() > 0);
        }
        Err(e) => {
            println!("❌ Proof generation failed: {}", e);
            panic!("Proof generation failed");
        }
    }
}
