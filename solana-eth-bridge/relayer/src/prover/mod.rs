pub mod types;

#[cfg(test)]
mod tests;

use sp1_sdk::{ProverClient, SP1Stdin, SP1ProvingKey};
use std::time::Instant;
use types::{
    EthereumBlockData, ProofResult, ProofType, ProverError, 
    SolanaBlockData,
};

/// SP1 证明生成器
pub struct Sp1Prover {
    client: ProverClient,
    solana_pk: SP1ProvingKey,
    eth_pk: SP1ProvingKey,
}

impl Sp1Prover {
    /// 创建新的证明生成器
    pub fn new() -> Self {
        let client = ProverClient::new();
        
        // 从构建产物中加载 ELF 文件
        let solana_elf = include_bytes!(
            "../../../sp1-programs/solana-verifier/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/solana-verifier"
        );
        let eth_elf = include_bytes!(
            "../../../sp1-programs/eth-verifier/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eth-verifier"
        );
        
        // 创建 ProvingKey
        let (solana_pk, _solana_vk) = client.setup(solana_elf);
        let (eth_pk, _eth_vk) = client.setup(eth_elf);
        
        println!("🔧 SP1 Prover initialized");
        println!("   • Solana verifier ELF: {} bytes", solana_elf.len());
        println!("   • Ethereum verifier ELF: {} bytes", eth_elf.len());
        
        Self {
            client,
            solana_pk,
            eth_pk,
        }
    }
    
    /// 为 Solana 区块生成 ZK 证明
    /// 
    /// 生成 STARK 证明并压缩为 Groth16 (用于 Ethereum 链上验证)
    pub fn prove_solana_block(
        &self,
        block_data: &SolanaBlockData,
    ) -> Result<ProofResult, ProverError> {
        println!("🔬 Generating proof for Solana block {}", block_data.slot);
        let start = Instant::now();
        
        // 1. 准备输入数据
        let mut stdin = SP1Stdin::new();
        
        // 序列化区块数据并写入 stdin
        let encoded = bincode::serialize(block_data)?;
        stdin.write_slice(&encoded);
        
        println!("   📝 Input data prepared: {} bytes", encoded.len());
        
        // 2. 生成 STARK 证明
        println!("   ⚡ Generating STARK proof...");
        let stark_start = Instant::now();
        
        let proof = self.client
            .prove(&self.solana_pk, stdin)
            .run()
            .map_err(|e| ProverError::ProofGeneration(e.to_string()))?;
        
        let stark_time = stark_start.elapsed();
        println!("   ✅ STARK proof generated in {:.2}s", stark_time.as_secs_f64());
        
        // 3. 压缩为 Groth16
        println!("   🗜️  Compressing to Groth16...");
        let compress_start = Instant::now();
        
        let compressed = self.client
            .prove(&self.solana_pk, SP1Stdin::new())
            .groth16()
            .run()
            .map_err(|e| ProverError::ProofCompression(e.to_string()))?;
        
        let compress_time = compress_start.elapsed();
        println!("   ✅ Groth16 proof compressed in {:.2}s", compress_time.as_secs_f64());
        
        // 4. 提取证明和公开值
        let proof_bytes = compressed.bytes();
        let public_values = proof.public_values.to_vec();
        
        let total_time = start.elapsed();
        println!("   🎉 Total proof generation: {:.2}s", total_time.as_secs_f64());
        println!("   📦 Proof size: {} bytes", proof_bytes.len());
        
        Ok(ProofResult {
            proof_bytes,
            public_values,
            proof_type: ProofType::Groth16,
        })
    }
    
    /// 为 Ethereum 区块生成 ZK 证明
    /// 
    /// 生成 STARK 证明 (用于 Solana 链上验证)
    pub fn prove_ethereum_block(
        &self,
        block_data: &EthereumBlockData,
    ) -> Result<ProofResult, ProverError> {
        println!("🔬 Generating proof for Ethereum block {}", block_data.number);
        let start = Instant::now();
        
        // 1. 准备输入数据
        let mut stdin = SP1Stdin::new();
        
        // 序列化区块数据
        let encoded = bincode::serialize(block_data)?;
        stdin.write_slice(&encoded);
        
        println!("   📝 Input data prepared: {} bytes", encoded.len());
        
        // 2. 生成 STARK 证明
        println!("   ⚡ Generating STARK proof...");
        
        let proof = self.client
            .prove(&self.eth_pk, stdin)
            .run()
            .map_err(|e| ProverError::ProofGeneration(e.to_string()))?;
        
        let total_time = start.elapsed();
        println!("   ✅ STARK proof generated in {:.2}s", total_time.as_secs_f64());
        
        // 3. 提取证明和公开值
        let proof_bytes = proof.bytes();
        let public_values = proof.public_values.to_vec();
        
        println!("   📦 Proof size: {} bytes", proof_bytes.len());
        
        Ok(ProofResult {
            proof_bytes,
            public_values,
            proof_type: ProofType::Stark,
        })
    }
}

impl Default for Sp1Prover {
    fn default() -> Self {
        Self::new()
    }
}
