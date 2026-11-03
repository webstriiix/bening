use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::{MAX_LABEL_LEN, MAX_MILESTONES, MAX_TITLE_LEN, MAX_URI_LEN};
use crate::error::BeningError;
use crate::state::{Campaign, CampaignStatus, Milestone, MilestoneInput, MilestoneStatus};

#[derive(Accounts)]
#[instruction(campaign_id: u64)]
pub struct CreateCampaign<'info> {
    #[account(mut)]
    pub organizer: Signer<'info>,

    #[account(
        init,
        payer = organizer,
        space = 8 + Campaign::INIT_SPACE,
        seeds = [b"campaign", organizer.key().as_ref(), &campaign_id.to_le_bytes()],
        bump
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = organizer,
        associated_token::mint = mint,
        associated_token::authority = campaign,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handle_create_campaign(
    ctx: Context<CreateCampaign>,
    campaign_id: u64,
    title: String,
    metadata_uri: String,
    verifier: Pubkey,
    milestones: Vec<MilestoneInput>,
) -> Result<()> {
    let organizer_key = ctx.accounts.organizer.key();

    require!(!milestones.is_empty(), BeningError::InvalidMilestoneCount);
    require!(
        (milestones.len() as u64) <= MAX_MILESTONES,
        BeningError::MaxMilestonesExceeded
    );
    require!(
        !title.is_empty() && title.len() <= MAX_TITLE_LEN as usize,
        BeningError::InvalidTitleLength
    );
    require!(
        metadata_uri.len() <= MAX_URI_LEN as usize,
        BeningError::InvalidMetadataUriLength
    );
    require!(
        verifier != organizer_key && verifier != Pubkey::default(),
        BeningError::InvalidVerifier
    );

    let mut target_amount: u64 = 0;
    let mut stored: Vec<Milestone> = Vec::with_capacity(milestones.len());
    for input in milestones.into_iter() {
        require!(input.amount > 0, BeningError::InvalidAmount);
        require!(
            input.label.len() <= MAX_LABEL_LEN as usize,
            BeningError::InvalidLabelLength
        );
        require!(
            input.vendor != organizer_key
                && input.vendor != verifier
                && input.vendor != Pubkey::default(),
            BeningError::InvalidVendor
        );
        target_amount = target_amount
            .checked_add(input.amount)
            .ok_or(BeningError::TargetAmountOverflow)?;
        stored.push(Milestone {
            label: input.label,
            amount: input.amount,
            vendor: input.vendor,
            status: MilestoneStatus::Pending,
            proof_hash: [0u8; 32],
            proof_uri: String::new(),
            proof_submitted_at: 0,
            approved_at: 0,
            released_at: 0,
        });
    }

    let campaign = &mut ctx.accounts.campaign;
    campaign.organizer = organizer_key;
    campaign.verifier = verifier;
    campaign.mint = ctx.accounts.mint.key();
    campaign.campaign_id = campaign_id;
    campaign.title = title;
    campaign.metadata_uri = metadata_uri;
    campaign.target_amount = target_amount;
    campaign.total_raised = 0;
    campaign.total_released = 0;
    campaign.donation_count = 0;
    campaign.status = CampaignStatus::Active;
    campaign.milestones = stored;
    campaign.bump = ctx.bumps.campaign;

    Ok(())
}
