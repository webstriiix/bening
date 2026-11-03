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
}
