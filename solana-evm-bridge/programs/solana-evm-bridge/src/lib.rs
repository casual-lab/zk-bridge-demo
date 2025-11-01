use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("GbtjEQYnuvVKN5DiQjvqoPGA9vS2tsH7mTfS6SJZXgBf");

#[program]
pub mod solana_evm_bridge {
    use super::*;

    pub fn initialize_bridge(
        ctx: Context<InitializeBridge>,
        evm_chain_id: u64,
    ) -> Result<()> {
        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.admin = ctx.accounts.admin.key();
        bridge_config.evm_chain_id = evm_chain_id;
        bridge_config.paused = false;
        bridge_config.next_order_id = 1;
        
        // Phase 1.4: Initialize relayer fee configuration
        bridge_config.relayer_fee_bps = 10;          // 0.1% default
        bridge_config.min_relayer_fee = 50_000;      // 0.05 USDC (6 decimals)
        
        emit!(BridgeInitialized {
            admin: bridge_config.admin,
            evm_chain_id,
            relayer_fee_bps: bridge_config.relayer_fee_bps,
        });
        
        msg!("Bridge initialized with relayer fee: {} bps", bridge_config.relayer_fee_bps);
        Ok(())
    }

    pub fn register_token_pair(
        ctx: Context<RegisterTokenPair>,
        evm_token: [u8; 20],
        is_native_solana: bool,
    ) -> Result<()> {
        let token_config = &mut ctx.accounts.token_config;
        token_config.solana_mint = ctx.accounts.solana_mint.key();
        token_config.evm_token = evm_token;
        token_config.is_native_solana = is_native_solana;
        token_config.total_locked = 0;
        
        msg!("Token pair registered");
        Ok(())
    }
    
    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        msg!("Vault initialized for token: {}", ctx.accounts.solana_mint.key());
        Ok(())
    }

    pub fn lock_tokens(
        ctx: Context<LockTokens>,
        amount: u64,
        recipient_evm: [u8; 20],
    ) -> Result<()> {
        require!(!ctx.accounts.bridge_config.paused, BridgeError::BridgePaused);
        require!(amount > 0, BridgeError::InvalidAmount);
        
        let bridge_config = &mut ctx.accounts.bridge_config;
        let token_config = &mut ctx.accounts.token_config;
        let order = &mut ctx.accounts.order;
        
        // Calculate relayer fee
        let relayer_fee = (amount as u128)
            .checked_mul(bridge_config.relayer_fee_bps as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
        
        let amount_to_lock = amount.checked_sub(relayer_fee).unwrap();
        
        // Transfer tokens from user to vault using CPI
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token::transfer(transfer_ctx, amount_to_lock)?;
        
        // Create order
        let clock = Clock::get()?;
        order.order_id = bridge_config.next_order_id;
        order.user = ctx.accounts.user.key();
        order.source_chain = 0;
        order.token_config = token_config.key();
        order.amount = amount_to_lock;
        order.recipient = recipient_evm;
        order.relayer_fee = relayer_fee;
        order.created_slot = clock.slot;
        order.status = OrderStatus::Pending;
        order.proof_hash = [0u8; 32];
        // Phase 1.4: Initialize relayer tracking fields
        order.completed_by = Pubkey::default();
        order.completed_at = 0;
        
        token_config.total_locked = token_config.total_locked.checked_add(amount_to_lock).unwrap();
        bridge_config.next_order_id = bridge_config.next_order_id.checked_add(1).unwrap();
        
        emit!(TokensLocked {
            order_id: order.order_id,
            user: order.user,
            amount: amount_to_lock,
            recipient: recipient_evm,
            relayer_fee,
            slot: clock.slot,
        });
        
        msg!("Tokens locked");
        Ok(())
    }
    
    pub fn unlock_tokens(
        ctx: Context<UnlockTokens>,
        order_id: u64,
        proof_hash: [u8; 32],
    ) -> Result<()> {
        require!(!ctx.accounts.bridge_config.paused, BridgeError::BridgePaused);
        
        let order = &mut ctx.accounts.order;
        let token_config = &mut ctx.accounts.token_config;
        let bridge_config = &ctx.accounts.bridge_config;
        let clock = Clock::get()?;
        
        // Verify order status (critical: prevents double unlock)
        require!(
            order.status == OrderStatus::Pending,
            BridgeError::OrderNotPending
        );
        
        // Verify order_id matches
        require!(order.order_id == order_id, BridgeError::OrderNotFound);
        
        // Phase 1.4: Verify ZK proof (mock verification for now, Phase 6 will use real SP1)
        require!(
            proof_hash != [0u8; 32],
            BridgeError::InvalidProof
        );
        
        // Phase 1.4: Calculate relayer fee
        let total_amount = order.amount;
        let relayer_fee = calculate_relayer_fee(
            total_amount,
            bridge_config.relayer_fee_bps,
            bridge_config.min_relayer_fee,
        );
        let user_amount = total_amount.checked_sub(relayer_fee).unwrap();
        
        // Verify amount is sufficient
        require!(
            total_amount >= relayer_fee,
            BridgeError::InsufficientAmount
        );
        
        // Transfer tokens from vault to user
        let vault_seeds = &[
            b"vault",
            token_config.solana_mint.as_ref(),
            &[ctx.bumps.vault],
        ];
        let vault_signer = &[&vault_seeds[..]];
        
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                vault_signer,
            ),
            user_amount,
        )?;
        
        // Transfer relayer fee
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.relayer_reward_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                vault_signer,
            ),
            relayer_fee,
        )?;
        
        // Update order status
        order.status = OrderStatus::Completed;
        order.completed_by = ctx.accounts.relayer.key();
        order.completed_at = clock.slot;
        order.proof_hash = proof_hash;
        
        // Update token config
        token_config.total_locked = token_config.total_locked.checked_sub(total_amount).unwrap();
        
        emit!(TokensUnlocked {
            order_id,
            user: order.user,
            amount: user_amount,
            relayer: ctx.accounts.relayer.key(),
            relayer_fee,
            slot: clock.slot,
        });
        
        msg!("Tokens unlocked successfully");
        Ok(())
    }
}

// Helper function to calculate relayer fee
fn calculate_relayer_fee(amount: u64, fee_bps: u16, min_fee: u64) -> u64 {
    let percentage_fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .unwrap()
        .checked_div(10000)
        .unwrap() as u64;
    
    // Return the maximum of percentage fee and minimum fee
    percentage_fee.max(min_fee)
}

// Accounts
#[derive(Accounts)]
pub struct InitializeBridge<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 8 + 1 + 8 + 2 + 8,
        seeds = [b"bridge_config"],
        bump
    )]
    pub bridge_config: Account<'info, BridgeConfig>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterTokenPair<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 20 + 1 + 8,
        seeds = [b"token_config", solana_mint.key().as_ref()],
        bump
    )]
    pub token_config: Account<'info, TokenConfig>,
    
    pub solana_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = admin,
        token::mint = solana_mint,
        token::authority = vault,
        seeds = [b"vault", solana_mint.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,
    
    pub solana_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64, recipient_evm: [u8; 20])]
pub struct LockTokens<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 8 + 32 + 1 + 32 + 32 + 8 + 20 + 8 + 8 + 8 + 32 + 32 + 8,
        seeds = [b"transfer_order", bridge_config.next_order_id.to_le_bytes().as_ref()],
        bump
    )]
    pub order: Account<'info, TransferOrder>,
    
    #[account(mut)]
    pub bridge_config: Account<'info, BridgeConfig>,
    
    #[account(mut)]
    pub token_config: Account<'info, TokenConfig>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(
        mut,
        constraint = user_token_account.owner == user.key(),
        constraint = user_token_account.mint == token_config.solana_mint
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        seeds = [b"vault", token_config.solana_mint.as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(order_id: u64)]
pub struct UnlockTokens<'info> {
    #[account(
        mut,
        seeds = [b"transfer_order", order_id.to_le_bytes().as_ref()],
        bump,
        constraint = order.status == OrderStatus::Pending @ BridgeError::OrderNotPending,
    )]
    pub order: Account<'info, TransferOrder>,
    
    #[account(mut)]
    pub bridge_config: Account<'info, BridgeConfig>,
    
    #[account(mut)]
    pub token_config: Account<'info, TokenConfig>,
    
    #[account(
        mut,
        constraint = user_token_account.owner == order.user,
        constraint = user_token_account.mint == token_config.solana_mint
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        seeds = [b"vault", token_config.solana_mint.as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,
    
    pub relayer: Signer<'info>,
    
    #[account(
        mut,
        constraint = relayer_reward_account.owner == relayer.key(),
        constraint = relayer_reward_account.mint == token_config.solana_mint
    )]
    pub relayer_reward_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

// Data structures
#[account]
pub struct BridgeConfig {
    pub admin: Pubkey,
    pub evm_chain_id: u64,
    pub paused: bool,
    pub next_order_id: u64,
    pub relayer_fee_bps: u16,
    pub min_relayer_fee: u64,
}

#[account]
pub struct TokenConfig {
    pub solana_mint: Pubkey,
    pub evm_token: [u8; 20],
    pub is_native_solana: bool,
    pub total_locked: u64,
}

#[account]
pub struct TransferOrder {
    pub order_id: u64,
    pub user: Pubkey,
    pub status: OrderStatus,
    pub token_config: Pubkey,
    pub source_chain: u8,
    pub amount: u64,
    pub recipient: [u8; 20],
    pub relayer_fee: u64,
    pub created_slot: u64,
    pub proof_hash: [u8; 32],
    pub completed_by: Pubkey,
    pub completed_at: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Completed,
}

// Events
#[event]
pub struct BridgeInitialized {
    pub admin: Pubkey,
    pub evm_chain_id: u64,
    pub relayer_fee_bps: u16,
}

#[event]
pub struct TokensLocked {
    pub order_id: u64,
    pub user: Pubkey,
    pub amount: u64,
    pub recipient: [u8; 20],
    pub relayer_fee: u64,
    pub slot: u64,
}

#[event]
pub struct TokensUnlocked {
    pub order_id: u64,
    pub user: Pubkey,
    pub amount: u64,
    pub relayer: Pubkey,
    pub relayer_fee: u64,
    pub slot: u64,
}

// Errors
#[error_code]
pub enum BridgeError {
    #[msg("Bridge is paused")]
    BridgePaused,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Order not found")]
    OrderNotFound,
    #[msg("Order not in pending status")]
    OrderNotPending,
    #[msg("Invalid proof")]
    InvalidProof,
    #[msg("Insufficient amount for relayer fee")]
    InsufficientAmount,
}
