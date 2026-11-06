use anchor_lang::prelude::*;
use anchor_spl::{
    token_interface::{transfer_checked, TokenAccount, TokenInterface},
};

use crate::{
    error::BeningError,
    state::{Campaign, CampaignStatus, MilestoneStatus},
};

#[derive(Accounts)]
pub struct ReleaseFunds<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [b"campaign", campaign.organizer.as_ref(), &campaign.campaign_id.to_le_bytes()],
        bump = campaign.bump,
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(address = campaign.mint @ BeningError::InvalidMint)]
    pub mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = campaign,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub vendor_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(address = anchor_spl::token::ID)]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_release_funds(ctx: Context<ReleaseFunds>, milestone_index: u8) -> Result<()> {
    // Extract all needed campaign values before any mutable borrow
    let campaign_mint = ctx.accounts.campaign.mint;
    let campaign_organizer = ctx.accounts.campaign.organizer;
    let campaign_id = ctx.accounts.campaign.campaign_id;
    let campaign_bump = ctx.accounts.campaign.bump;

    let campaign = &mut ctx.accounts.campaign;

    // 1. milestone_index in range
    if milestone_index as usize >= campaign.milestones.len() {
        return err!(BeningError::InvalidMilestoneIndex);
    }

    let milestone = &mut campaign.milestones[milestone_index as usize];

    // 2. milestone status == Approved
    if milestone.status != MilestoneStatus::Approved {
        return err!(BeningError::InvalidStatus);
    }

    // 3. vendor ATA mint == campaign.mint (enforced by associated_token::mint = mint constraint below)

    // 4. vendor ATA owner == milestone.vendor (enforced in handler with require!)
    require!(
        ctx.accounts.vendor_token_account.mint == campaign_mint,
        BeningError::InvalidVendorAccount
    );
    require!(
        ctx.accounts.vendor_token_account.owner == milestone.vendor,
        BeningError::InvalidVendorAccount
    );

    // 5. vault balance >= milestone.amount
    let vault_balance = ctx.accounts.vault.amount;
    let amount = milestone.amount;
    if vault_balance < amount {
        return err!(BeningError::InsufficientVaultBalance);
    }

    // Update state BEFORE CPI (SEC ordering)
    milestone.status = MilestoneStatus::Released;
    milestone.released_at = Clock::get()?.unix_timestamp;

    // Checked add
    campaign.total_released = campaign.total_released.checked_add(amount).ok_or(BeningError::TargetAmountOverflow)?;

    // If all milestones Released, campaign status = Completed
    let all_released = campaign.milestones.iter().all(|m| m.status == MilestoneStatus::Released);
    if all_released {
        campaign.status = CampaignStatus::Completed;
    }

    // Build CPI signer seeds from extracted local values
    let seeds: [&[u8]; 4] = [
        b"campaign",
        campaign_organizer.as_ref(),
        &campaign_id.to_le_bytes(),
        &[campaign_bump],
    ];
    let signer_seeds: [&[&[u8]]; 1] = [&seeds];

    let cpi_accounts = anchor_spl::token_interface::TransferChecked {
        from: ctx.accounts.vault.to_account_info(),
        to: ctx.accounts.vendor_token_account.to_account_info(),
        authority: ctx.accounts.campaign.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        cpi_accounts,
        &signer_seeds,
    );
    transfer_checked(cpi_ctx, amount, ctx.accounts.mint.decimals)?;

    Ok(())
}