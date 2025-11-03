// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./ISP1Verifier.sol";

/// @title MockSP1Verifier
/// @notice Mock implementation of SP1 verifier for development and testing
/// @dev In production, replace with real SP1 Groth16 verifier address
contract MockSP1Verifier is ISP1Verifier {
    // Events
    event ProofVerified(bytes32 indexed programVKey, bytes publicValues, uint256 proofLength);
    event VerificationFailed(bytes32 indexed programVKey, string reason);
    
    // State
    bool public strictMode; // If true, performs basic validation
    mapping(bytes32 => bool) public approvedPrograms;
    uint256 public verificationCount;
    
    constructor(bool _strictMode) {
        strictMode = _strictMode;
    }
    
    /// @notice Approve a program verification key
    /// @param programVKey The program VKey to approve
    function approveProgram(bytes32 programVKey) external {
        approvedPrograms[programVKey] = true;
    }
    
    /// @notice Mock verification - always succeeds in non-strict mode
    /// @param programVKey The verification key of the SP1 program
    /// @param publicValues The public outputs of the program
    /// @param proofBytes The Groth16 proof bytes
    function verifyProof(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external override {
        if (strictMode) {
            // Basic validation in strict mode
            require(programVKey != bytes32(0), "MockSP1Verifier: Invalid program VKey");
            require(publicValues.length > 0, "MockSP1Verifier: Empty public values");
            require(proofBytes.length > 0, "MockSP1Verifier: Empty proof");
            
            // Check if program is approved (optional)
            if (approvedPrograms[programVKey]) {
                // Approved program - allow
            } else {
                // For mock, we still allow but could revert in production
            }
        }
        
        emit ProofVerified(programVKey, publicValues, proofBytes.length);
        
        // Mock: always succeeds
        // In production, this would verify the actual Groth16 proof
    }
}
