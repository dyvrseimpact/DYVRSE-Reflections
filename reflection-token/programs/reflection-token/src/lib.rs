use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

// Constants
const REFLECTION_TAX_BPS: u64 = 300; // 3% tax (basis points)
const DECIMALS: u64 = 1_000_000_000; // Assuming 9 decimal places for SPL tokens
const BATCH_SIZE: usize = 500; // Maximum batch processing size

#[program]
pub mod dyvrse_reflections {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let token_state = &mut ctx.accounts.token_state;
        token_state.tax_pool = 0;
        token_state.processing = false;
        token_state.exclusion_list = Vec::new();
        Ok(())
    }

    pub fn transfer(ctx: Context<CustomTransfer>, amount: u64) -> Result<()> {
        let token_state = &mut ctx.accounts.token_state;
        let tax_amount = calculate_tax(amount, REFLECTION_TAX_BPS);
        let final_amount = amount.saturating_sub(tax_amount);

        token_state.tax_pool += tax_amount;

        token::transfer(ctx.accounts.transfer_ctx(), final_amount)?;
        Ok(())
    }

    pub fn distribute_reflections(ctx: Context<DistributeReflections>, start: u64, end: u64) -> Result<()> {
        let token_state = &mut ctx.accounts.token_state;
        require!(!token_state.processing, CustomError::ReentrancyDetected);
        token_state.processing = true;

        let total_supply = ctx.accounts.mint.supply;
        let tax_pool = token_state.tax_pool;

        if tax_pool > 0 {
            let mut processed = 0;
            for i in start..end {
                if let Some(holder) = ctx.accounts.holder_accounts.get(i as usize) {
                    if !token_state.exclusion_list.contains(&holder.owner) {
                        let share = (holder.amount as u128 * tax_pool as u128) / total_supply as u128;
                        token::transfer(ctx.accounts.holder_transfer_ctx(holder), share as u64)?;
                        processed += 1;
                    }
                }
            }
            token_state.tax_pool -= processed as u64;
        }
        token_state.processing = false;
        Ok(())
    }

    pub fn exclude_address(ctx: Context<Exclude>, account: Pubkey) -> Result<()> {
        let token_state = &mut ctx.accounts.token_state;
        token_state.exclusion_list.push(account);
        Ok(())
    }
}

#[account]
pub struct TokenState {
    pub tax_pool: u64,
    pub processing: bool,
    pub exclusion_list: Vec<Pubkey>,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = authority, space = 8 + 40)]
    pub token_state: Account<'info, TokenState>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CustomTransfer<'info> {
    #[account(mut)]
    pub token_state: Account<'info, TokenState>,
    #[account(mut)]
    pub from: Signer<'info>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct DistributeReflections<'info> {
    #[account(mut)]
    pub token_state: Account<'info, TokenState>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub holder_accounts: Vec<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Exclude<'info> {
    #[account(mut)]
    pub token_state: Account<'info, TokenState>,
    pub authority: Signer<'info>,
}

#[error_code]
pub enum CustomError {
    #[msg("Reentrancy detected")] ReentrancyDetected,
}

pub fn calculate_tax(amount: u64, tax_rate: u64) -> u64 {
    ((amount as u128 * tax_rate as u128) / 10_000) as u64 // Uses basis points (BPS)
}
