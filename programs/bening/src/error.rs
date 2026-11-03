use anchor_lang::prelude::*;

// Failure -> variant mapping (FR-7):
// - milestones.len() == 0                                   -> InvalidMilestoneCount
// - milestones.len() > MAX_MILESTONES                       -> MaxMilestonesExceeded
// - title.len() == 0 or > MAX_TITLE_LEN                     -> InvalidTitleLength
// - label.len() > MAX_LABEL_LEN                             -> InvalidLabelLength
// - metadata_uri.len() > MAX_URI_LEN                        -> InvalidMetadataUriLength
// - verifier == organizer or verifier == Pubkey::default()  -> InvalidVerifier
// - milestone.vendor == organizer/verifier/default          -> InvalidVendor
// - milestone.amount == 0                                   -> InvalidAmount
// - checked_add overflow while summing milestone amounts    -> TargetAmountOverflow
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
}
