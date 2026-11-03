use anchor_lang::prelude::*;

#[derive(InitSpace, Default, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum CampaignStatus {
    #[default]
    Active,
    Completed,
}

#[derive(InitSpace, Default, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum MilestoneStatus {
    #[default]
    Pending,
    ProofSubmitted,
    Approved,
    Released,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct MilestoneInput {
    pub label: String,
    pub amount: u64,
    pub vendor: Pubkey,
}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct Milestone {
    #[max_len(32)]
    pub label: String,
    pub amount: u64,
    pub vendor: Pubkey,
    pub status: MilestoneStatus,
    pub proof_hash: [u8; 32],
    #[max_len(128)]
    pub proof_uri: String,
    pub proof_submitted_at: i64,
    pub approved_at: i64,
    pub released_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct Campaign {
    pub organizer: Pubkey,
    pub verifier: Pubkey,
    pub mint: Pubkey,
    pub campaign_id: u64,
    #[max_len(48)]
    pub title: String,
    #[max_len(128)]
    pub metadata_uri: String,
    pub target_amount: u64,
    pub total_raised: u64,
    pub total_released: u64,
    pub donation_count: u32,
    pub status: CampaignStatus,
    #[max_len(5)]
    pub milestones: Vec<Milestone>,
    pub bump: u8,
}
