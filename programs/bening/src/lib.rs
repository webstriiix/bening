pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("3uemNohfUixociV872GGK1e4Dh2PRbPiNYT2SMgKXh5y");

#[program]
pub mod bening {
    use super::*;

    pub fn create_campaign(
        ctx: Context<CreateCampaign>,
        campaign_id: u64,
        title: String,
        metadata_uri: String,
        verifier: Pubkey,
        milestones: Vec<MilestoneInput>,
    ) -> Result<()> {
        handle_create_campaign(ctx, campaign_id, title, metadata_uri, verifier, milestones)
    }

    pub fn donate(
        ctx: Context<Donate>,
        amount: u64,
    ) -> Result<()> {
        handle_donate(ctx, amount)
    }

    pub fn submit_proof(
        ctx: Context<SubmitProof>,
        milestone_index: u8,
        proof_hash: [u8; 32],
        proof_uri: String,
    ) -> Result<()> {
        handle_submit_proof(ctx, milestone_index, proof_hash, proof_uri)
    }

    pub fn approve_milestone(
        ctx: Context<ApproveMilestone>,
        milestone_index: u8,
    ) -> Result<()> {
        handle_approve_milestone(ctx, milestone_index)
    }

    pub fn release_funds(
        ctx: Context<ReleaseFunds>,
        milestone_index: u8,
    ) -> Result<()> {
        handle_release_funds(ctx, milestone_index)
    }
}