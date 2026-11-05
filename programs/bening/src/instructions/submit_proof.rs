use anchor_lang::prelude::*;
use crate::error::BeningError;
use crate::state::{Campaign, MilestoneStatus};

const MAX_URI_LEN: usize = 128;

#[derive(Accounts)]
pub struct SubmitProof<'info> {
    pub organizer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"campaign", campaign.organizer.as_ref(), &campaign.campaign_id.to_le_bytes()],
        bump = campaign.bump,
        has_one = organizer @ BeningError::NotOrganizer
    )]
    pub campaign: Account<'info, Campaign>,
}

pub fn handle_submit_proof(
    ctx: Context<SubmitProof>,
    milestone_index: u8,
    proof_hash: [u8; 32],
    proof_uri: String,
) -> Result<()> {
    let campaign = &mut ctx.accounts.campaign;

    let index = milestone_index as usize;
    require!(index < campaign.milestones.len(), BeningError::InvalidMilestoneIndex);

    let milestone = &mut campaign.milestones[index];
    require!(milestone.status == MilestoneStatus::Pending, BeningError::InvalidStatus);

    require!(
        !proof_hash.iter().all(|&b| b == 0),
        BeningError::InvalidProofHash
    );
    require!(
        proof_uri.len() <= MAX_URI_LEN as usize,
        BeningError::InvalidUriLength
    );

    milestone.status = MilestoneStatus::ProofSubmitted;
    milestone.proof_hash = proof_hash;
    milestone.proof_uri = proof_uri;
    milestone.proof_submitted_at = Clock::get()?.unix_timestamp;

    Ok(())
}