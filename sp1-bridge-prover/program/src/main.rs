//! Bridge Order Verification Guest Program
//! 
//! This program verifies a cross-chain transfer order against a Merkle proof
//! and outputs the verified order details as public values.

#![no_main]
sp1_zkvm::entrypoint!(main);

mod bridge_verify;

pub fn main() {
    bridge_verify::verify_bridge_order();
}
