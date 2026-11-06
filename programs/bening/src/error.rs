use anchor_lang::prelude::*;

#[error_code]
pub enum BeningError {
    #[msg("Maximum milestones exceeded")]
    MaxMilestonesExceeded,
    #[msg("Title length must be between 1 and MAX_TITLE_LEN")]
    InvalidTitleLength,
    #[msg("Label length must be <= MAX_LABEL_LEN")]
    InvalidLabelLength,
    #[msg("Metadata URI length must be <= MAX_URI_LEN")]
    InvalidMetadataUriLength,
    #[msg("Verifier cannot be organizer or default pubkey")]
    InvalidVerifier,
    #[msg("Milestone vendor cannot be organizer, verifier, or default pubkey")]
    InvalidVendor,
    #[msg("Milestone amount must be > 0")]
    InvalidAmount,
    #[msg("Milestone count must be between 1 and MAX_MILESTONES")]
    InvalidMilestoneCount,
    #[msg("Target amount calculation overflow")]
    TargetAmountOverflow,
    #[msg("Campaign is not active")]
    CampaignInactive,
    #[msg("Donation overflow: total raised would exceed target")]
    DonationOverflow,
    #[msg("Donation exceeds target amount")]
    DonationBeyondTarget,
    #[msg("Invalid mint or donor token account owner mismatch")]
    InvalidMint,
    #[msg("Only the organizer may submit a proof")]
    NotOrganizer,
    #[msg("Only the verifier may approve")]
    NotVerifier,
    #[msg("Milestone index out of range")]
    InvalidMilestoneIndex,
    #[msg("Milestone is not in the expected status")]
    InvalidStatus,
    #[msg("Proof hash must not be all zeros")]
    InvalidProofHash,
    #[msg("Proof URI length must be <= MAX_URI_LEN")]
    InvalidUriLength,
    #[msg("Vendor token account mismatch (owner or mint)")]
    InvalidVendorAccount,
    #[msg("Vault does not have sufficient balance")]
    InsufficientVaultBalance,
    #[msg("Campaign status is not Released (reverted)")]
    CampaignNotReleased,
}