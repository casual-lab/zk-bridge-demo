fn main() {
    // SP1 程序需要单独使用 `cargo prove build` 构建
    // 构建命令:
    // cd ../sp1-programs/solana-verifier && cargo prove build
    // cd ../sp1-programs/eth-verifier && cargo prove build
    
    println!("cargo:rerun-if-changed=../sp1-programs/solana-verifier/src");
    println!("cargo:rerun-if-changed=../sp1-programs/eth-verifier/src");
    
    println!("✅ Build script completed. Make sure to build SP1 programs separately!");
}
