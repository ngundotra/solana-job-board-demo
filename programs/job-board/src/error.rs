use anchor_lang::prelude::*;

#[error_code]
pub enum JobBoardError {
    #[msg("Bounty key is non-null but not provided with instruction")]
    BountyAccountMissing,

    #[msg("Wrong account provided for bounty account")]
    BountyAccountMismatch,

    #[msg("Attempted to set yourself available while delist timeout is still active")]
    BountyDelistTimeoutStillActive,
}
