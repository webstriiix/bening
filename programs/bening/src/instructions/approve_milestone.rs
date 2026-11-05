use anchor_lang::prelude::*;
use crate::error::BeningError;
use crate::state::{Campaign, MilestoneStatus};

#[derive(Accounts)]
pub struct ApproveMilestone<'info> {
    pub verifier: Signer<'info>,

    #[account(
        mut,
        seeds = [b"campaign", campaign.organizer.as_ref(), &campaign.campaign_id.to_le_bytes()],
        bump = campaign.bump,
        has_one = verifier @ BeningError::NotVerifier
    )]
    pub campaign: Account<'info, Campaign>,
}

pub fn handle_approve_milestone(
    ctx: Context<ApproveMilestone>,
    milestone_index: u8,
) -> Result<()> {
    let campaign = &mut ctx.accounts.campaign;

    let index = milestone_index as usize;
    require!(index < campaign.milestones.len(), BeningError::InvalidMilestoneIndex);

    let milestone = &mut campaign.milestones[index];
    require!(milestone.status == MilestoneStatus::ProofSubmitted, BeningError::InvalidStatus);

    milestone.status = MilestoneStatus::Approved;
    milestone.approved_at = Clock::get()?.unix_timestamp;

    Ok(())
}