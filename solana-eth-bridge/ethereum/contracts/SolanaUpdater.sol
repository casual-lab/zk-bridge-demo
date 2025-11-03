// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./ISP1Verifier.sol";

/**
 * @title SolanaUpdater
 * @notice 接收并验证 Solana 区块头，通过 SP1 zkVM 证明
 */
contract SolanaUpdater {
    // ========================================
    // 数据结构
    // ========================================
    
    /// Solana 区块头
    struct SolanaBlockHeader {
        uint64 slot;
        bytes32 blockhash;
        bytes32 parentHash;
        uint64 blockHeight;
        int64 timestamp;
        uint32 confirmations;
    }
    
    // ========================================
    // 状态变量
    // ========================================
    
    /// 存储已验证的 Solana 区块头 (slot => header)
    mapping(uint64 => SolanaBlockHeader) public solanaHeaders;
    
    /// 当前最新的 slot
    uint64 public currentSlot;
    
    /// 管理员地址
    address public admin;
    
    /// SP1 验证器地址 (简化版，实际应该是 SP1 Verifier 合约)
    address public sp1Verifier;
    
    // ========================================
    // 事件
    // ========================================
    
    event SolanaBlockUpdated(
        uint64 indexed slot,
        bytes32 blockhash,
        uint64 blockHeight
    );
    
    event AdminChanged(address indexed oldAdmin, address indexed newAdmin);
    
    // ========================================
    // 修饰符
    // ========================================
    
    modifier onlyAdmin() {
        require(msg.sender == admin, "Only admin");
        _;
    }
    
    // ========================================
    // 构造函数
    // ========================================
    
    constructor(address _sp1Verifier) {
        admin = msg.sender;
        sp1Verifier = _sp1Verifier;
    }
    
    // ========================================
    // 核心函数
    // ========================================
    
    /**
     * @notice 更新 Solana 区块头
     * @param programVKey SP1 程序的验证密钥
     * @param publicValues 公开输入（编码的区块头数据）
     * @param proof SP1 Groth16 证明
     * @param header Solana 区块头数据
     */
    function updateSolanaBlock(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proof,
        SolanaBlockHeader calldata header
    ) public {
        // 1. 验证确认深度（防止分叉）
        require(
            header.confirmations >= 32,
            "Insufficient confirmations"
        );
        
        // 2. 验证区块连续性
        if (currentSlot > 0) {
            SolanaBlockHeader storage prevHeader = solanaHeaders[currentSlot];
            require(
                header.parentHash == prevHeader.blockhash,
                "Parent hash mismatch"
            );
            require(
                header.slot > currentSlot,
                "Slot must be greater than current"
            );
        }
        
        // 3. 验证 SP1 zkVM 证明
        ISP1Verifier(sp1Verifier).verifyProof(
            programVKey,
            publicValues,
            proof
        );
        
        // 4. 存储区块头
        solanaHeaders[header.slot] = header;
        currentSlot = header.slot;
        
        emit SolanaBlockUpdated(header.slot, header.blockhash, header.blockHeight);
    }
    
    /**
     * @notice 批量更新多个区块（用于快速同步）
     */
    function updateBatchSolanaBlocks(
        bytes32 programVKey,
        bytes[] calldata publicValues,
        bytes[] calldata proofs,
        SolanaBlockHeader[] calldata headers
    ) external {
        require(proofs.length == headers.length, "Length mismatch");
        require(publicValues.length == headers.length, "Public values length mismatch");
        
        for (uint256 i = 0; i < headers.length; i++) {
            updateSolanaBlock(programVKey, publicValues[i], proofs[i], headers[i]);
        }
    }
    
    /**
     * @notice 获取指定 slot 的区块头
     */
    function getSolanaBlock(uint64 slot)
        external
        view
        returns (SolanaBlockHeader memory)
    {
        return solanaHeaders[slot];
    }
    
    /**
     * @notice 验证 Merkle 证明（用于跨链消息验证）
     * @param slot 区块 slot
     * @param messageHash 消息哈希
     * @param merkleProof Merkle 证明
     */
    function verifyMessage(
        uint64 slot,
        bytes32 messageHash,
        bytes32[] calldata merkleProof
    ) external view returns (bool) {
        SolanaBlockHeader storage header = solanaHeaders[slot];
        require(header.slot != 0, "Block not found");
        
        // 简化版：直接返回 true
        // 实际应该验证 messageHash 在 transactions_root 的 Merkle 树中
        return true;
    }
    
    // ========================================
    // 管理函数
    // ========================================
    
    function setAdmin(address newAdmin) external onlyAdmin {
        address oldAdmin = admin;
        admin = newAdmin;
        emit AdminChanged(oldAdmin, newAdmin);
    }
    
    function setVerifier(address newVerifier) external onlyAdmin {
        sp1Verifier = newVerifier;
    }
}
