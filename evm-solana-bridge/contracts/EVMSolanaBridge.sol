// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title EVMSolanaBridge
 * @notice Phase 2: EVM side of the cross-chain bridge
 * @dev Symmetric implementation to Solana bridge contract (Phase 1.4)
 * 
 * Features:
 * - Lock/Unlock ERC20 tokens
 * - Relayer fee mechanism (0.1% with minimum)
 * - ZK proof verification for cross-chain validation
 * - Order status tracking (Pending -> Completed)
 * - No timeout refund (as per Phase 1.4 design)
 */
contract EVMSolanaBridge is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ============ Structs ============

    /// @notice Order status enum
    enum OrderStatus {
        Pending,
        Completed
    }

    /// @notice Token configuration for cross-chain pairs
    struct TokenConfig {
        address evmToken;           // ERC20 token address on EVM
        bytes32 solanaMint;         // Solana SPL token mint (32 bytes)
        bool isNativeEvm;           // true = lock/unlock, false = burn/mint
        uint256 totalLocked;        // Total amount locked in vault
    }

    /// @notice Transfer order tracking
    struct TransferOrder {
        uint64 orderId;             // Unique order ID
        address user;               // User who initiated the transfer
        uint8 sourceChain;          // 0 = Solana, 1 = EVM
        address tokenConfig;        // Reference to token config (EVM token address)
        uint256 amount;             // Amount locked (after fee deduction)
        bytes32 recipient;          // Recipient address on Solana (32 bytes)
        uint256 relayerFee;         // Fee reserved for relayer
        uint256 createdBlock;       // Block number when order was created
        OrderStatus status;         // Current order status
        bytes32 proofHash;          // ZK proof hash for verification
        address completedBy;        // Relayer who completed this order
        uint256 completedAt;        // Block number when completed
    }

    // ============ State Variables ============

    /// @notice Bridge configuration
    address public admin;
    bytes32 public solanaChainId;   // Solana chain identifier (can be a hash)
    bool public paused;
    uint64 public nextOrderId;

    /// @notice Relayer fee configuration (Phase 1.4)
    uint16 public relayerFeeBps;    // Fee in basis points (10 = 0.1%)
    uint256 public minRelayerFee;   // Minimum fee in token units (e.g., 50000 for 0.05 USDC with 6 decimals)

    /// @notice Token configurations: evmToken => TokenConfig
    mapping(address => TokenConfig) public tokenConfigs;

    /// @notice Transfer orders: orderId => TransferOrder
    mapping(uint64 => TransferOrder) public transferOrders;

    /// @notice Token vaults: evmToken => balance
    /// @dev Using contract's own balance tracking for safety
    mapping(address => uint256) public vaults;

    // ============ Events ============

    event BridgeInitialized(
        address indexed admin,
        bytes32 solanaChainId,
        uint16 relayerFeeBps
    );

    event TokenPairRegistered(
        address indexed evmToken,
        bytes32 solanaMint,
        bool isNativeEvm
    );

    event TokensLocked(
        uint64 indexed orderId,
        address indexed user,
        address indexed token,
        uint256 amount,
        uint256 relayerFee,
        bytes32 recipient
    );

    event TokensUnlocked(
        uint64 indexed orderId,
        address indexed relayer,
        address indexed user,
        address token,
        uint256 amount,
        uint256 relayerFee
    );

    event RelayerFeeUpdated(
        uint16 newFeeBps,
        uint256 newMinFee
    );

    event BridgePaused(bool isPaused);

    // ============ Errors ============

    error BridgeIsPaused();
    error InvalidAmount();
    error OrderNotPending();
    error InvalidProof();
    error UnauthorizedRelayer();
    error TokenNotRegistered();
    error InsufficientVaultBalance();

    // ============ Constructor ============

    constructor() Ownable(msg.sender) {
        admin = msg.sender;
        paused = false;
        nextOrderId = 1;
        
        // Phase 1.4: Initialize relayer fee configuration
        relayerFeeBps = 10;         // 0.1% default
        minRelayerFee = 50_000;     // 0.05 USDC (assuming 6 decimals)
    }

    // ============ Admin Functions ============

    /**
     * @notice Initialize bridge with Solana chain configuration
     * @param _solanaChainId Solana chain identifier (hash or other unique identifier)
     */
    function initializeBridge(bytes32 _solanaChainId) external onlyOwner {
        require(solanaChainId == bytes32(0), "Already initialized");
        
        solanaChainId = _solanaChainId;
        
        emit BridgeInitialized(admin, _solanaChainId, relayerFeeBps);
    }

    /**
     * @notice Register a token pair for cross-chain transfers
     * @param evmToken ERC20 token address on EVM
     * @param solanaMint SPL token mint on Solana (32 bytes)
     * @param isNativeEvm true = lock/unlock mode, false = burn/mint mode
     */
    function registerTokenPair(
        address evmToken,
        bytes32 solanaMint,
        bool isNativeEvm
    ) external onlyOwner {
        require(evmToken != address(0), "Invalid token address");
        require(tokenConfigs[evmToken].evmToken == address(0), "Token already registered");
        
        tokenConfigs[evmToken] = TokenConfig({
            evmToken: evmToken,
            solanaMint: solanaMint,
            isNativeEvm: isNativeEvm,
            totalLocked: 0
        });
        
        emit TokenPairRegistered(evmToken, solanaMint, isNativeEvm);
    }

    /**
     * @notice Update relayer fee configuration
     * @param newFeeBps New fee in basis points
     * @param newMinFee New minimum fee amount
     */
    function updateRelayerFee(uint16 newFeeBps, uint256 newMinFee) external onlyOwner {
        require(newFeeBps <= 10000, "Fee too high"); // Max 100%
        
        relayerFeeBps = newFeeBps;
        minRelayerFee = newMinFee;
        
        emit RelayerFeeUpdated(newFeeBps, newMinFee);
    }

    /**
     * @notice Pause or unpause the bridge
     * @param _paused New pause status
     */
    function setPaused(bool _paused) external onlyOwner {
        paused = _paused;
        emit BridgePaused(_paused);
    }

    // ============ User Functions ============

    /**
     * @notice Lock tokens on EVM to transfer to Solana
     * @param token ERC20 token address
     * @param amount Amount to lock (before fee deduction)
     * @param recipient Recipient address on Solana (32 bytes)
     * @return orderId The created order ID
     */
    function lockTokens(
        address token,
        uint256 amount,
        bytes32 recipient
    ) external nonReentrant returns (uint64 orderId) {
        if (paused) revert BridgeIsPaused();
        if (amount == 0) revert InvalidAmount();
        
        TokenConfig storage config = tokenConfigs[token];
        if (config.evmToken == address(0)) revert TokenNotRegistered();
        
        // Calculate relayer fee (percentage only, minimum applied at unlock)
        // This matches Solana contract behavior
        uint256 relayerFee = (amount * relayerFeeBps) / 10000;
        require(amount > relayerFee, "Amount too small");
        uint256 amountToLock = amount - relayerFee;
        
        // Transfer tokens from user to this contract
        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        
        // Update vault balance (only the locked amount, fee stays separate)
        vaults[token] += amountToLock;
        config.totalLocked += amountToLock;
        
        // Create order
        orderId = nextOrderId++;
        TransferOrder storage order = transferOrders[orderId];
        order.orderId = orderId;
        order.user = msg.sender;
        order.sourceChain = 1; // EVM
        order.tokenConfig = token;
        order.amount = amountToLock;
        order.recipient = recipient;
        order.relayerFee = relayerFee;
        order.createdBlock = block.number;
        order.status = OrderStatus.Pending;
        order.proofHash = bytes32(0);
        order.completedBy = address(0);
        order.completedAt = 0;
        
        emit TokensLocked(
            orderId,
            msg.sender,
            token,
            amountToLock,
            relayerFee,
            recipient
        );
        
        return orderId;
    }

    /**
     * @notice Unlock tokens after cross-chain transfer is verified
     * @param orderId Order ID to unlock
     * @param proofHash ZK proof hash for verification
     * @dev In Phase 1.4, proof verification is mocked (proof != 0x00...00)
     */
    function unlockTokens(
        uint64 orderId,
        bytes32 proofHash
    ) external nonReentrant {
        TransferOrder storage order = transferOrders[orderId];
        
        // Validate order status
        if (order.status != OrderStatus.Pending) revert OrderNotPending();
        
        // Mock ZK proof verification (Phase 1.4 approach)
        // In production, this would verify the actual ZK proof
        if (proofHash == bytes32(0)) revert InvalidProof();
        
        // Update order status
        order.status = OrderStatus.Completed;
        order.proofHash = proofHash;
        order.completedBy = msg.sender; // Relayer
        order.completedAt = block.number;
        
        TokenConfig storage config = tokenConfigs[order.tokenConfig];
        
        // Calculate relayer fee (matches Solana logic: max of percentage and minimum)
        uint256 totalAmount = order.amount;
        uint256 relayerReward = calculateRelayerFee(totalAmount);
        
        // Ensure sufficient amount
        require(totalAmount >= relayerReward, "Insufficient amount");
        
        uint256 userAmount = totalAmount - relayerReward;
        
        // Update vault balance
        vaults[order.tokenConfig] -= totalAmount;
        config.totalLocked -= totalAmount;
        
        // Transfer tokens
        IERC20(order.tokenConfig).safeTransfer(order.user, userAmount);
        IERC20(order.tokenConfig).safeTransfer(msg.sender, relayerReward);
        
        emit TokensUnlocked(
            orderId,
            msg.sender,
            order.user,
            order.tokenConfig,
            userAmount,
            relayerReward
        );
    }

    // ============ View Functions ============

    /**
     * @notice Calculate relayer fee for a given amount
     * @param amount Total amount before fee deduction
     * @return fee Calculated fee (max of percentage fee and minimum fee)
     */
    function calculateRelayerFee(uint256 amount) public view returns (uint256 fee) {
        fee = (amount * relayerFeeBps) / 10000;
        if (fee < minRelayerFee) {
            fee = minRelayerFee;
        }
        return fee;
    }

    /**
     * @notice Get token configuration
     * @param token ERC20 token address
     * @return config Token configuration
     */
    function getTokenConfig(address token) external view returns (TokenConfig memory config) {
        return tokenConfigs[token];
    }

    /**
     * @notice Get transfer order details
     * @param orderId Order ID
     * @return order Transfer order details
     */
    function getTransferOrder(uint64 orderId) external view returns (TransferOrder memory order) {
        return transferOrders[orderId];
    }

    /**
     * @notice Get vault balance for a token
     * @param token ERC20 token address
     * @return balance Vault balance
     */
    function getVaultBalance(address token) external view returns (uint256 balance) {
        return vaults[token];
    }
}
