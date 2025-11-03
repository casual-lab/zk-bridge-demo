// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title ISP1Verifier
/// @notice Interface for SP1 Groth16 proof verification
/// @dev This is the interface for SP1's on-chain verifier
interface ISP1Verifier {
    /// @notice Verifies a Groth16 proof
    /// @param programVKey The verification key of the SP1 program
    /// @param publicValues The public outputs of the program
    /// @param proofBytes The Groth16 proof bytes
    function verifyProof(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external;
}
