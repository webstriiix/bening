use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::error::BeningError;
use crate::state::{Campaign, CampaignStatus, Donation};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct Donate<'info> {
    #[account(mut)]
    pub donor: Signer<'info>,

    #[account(
        mut,
        constraint = campaign.status == CampaignStatus::Active @ BeningError::CampaignInactive
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = donor,
        space = 8 + Donation::INIT_SPACE,
        seeds = [b"donation", campaign.key().as_ref(), &campaign.donation_count.to_le_bytes()],
        bump
    )]
    pub donation: Account<'info, Donation>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = donor
    )]
    pub donor_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = campaign
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn handle_donate(ctx: Context<Donate>, amount: u64) -> Result<()> {
    let donor_key = ctx.accounts.donor.key();
    let campaign_key = ctx.accounts.campaign.key();

    // FR-2 validations
    require!(amount > 0, BeningError::InvalidAmount);

    let campaign = &mut ctx.accounts.campaign;
    let new_total = campaign
        .total_raised
        .checked_add(amount)
        .ok_or(BeningError::DonationOverflow)?;
    require!(new_total <= campaign.target_amount, BeningError::DonationBeyondTarget);

    // Check donor_ata mint matches campaign's expected mint
    require!(
        ctx.accounts.donor_ata.mint == campaign.mint,
        BeningError::InvalidMint
    );

    // Transfer donor -> vault via token program CPI (using transfer_checked)
    let cpi_accounts = anchor_spl::token::TransferChecked {
        from: ctx.accounts.donor_ata.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
        authority: ctx.accounts.donor.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.key();
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
    anchor_spl::token::transfer_checked(cpi_ctx, amount, ctx.accounts.mint.decimals)?;

    // Update campaign state
    campaign.total_raised = new_total;
    let donation_index = campaign.donation_count;
    campaign.donation_count = campaign
        .donation_count
        .checked_add(1)
        .ok_or(BeningError::DonationOverflow)?;

    // Save donation account data
    let donation = &mut ctx.accounts.donation;
    donation.campaign = campaign_key;
    donation.donor = donor_key;
    donation.amount = amount;
    donation.timestamp = ctx.accounts.clock.unix_timestamp;
    donation.index = donation_index;
    donation.bump = ctx.bumps.donation;

    Ok(())
}