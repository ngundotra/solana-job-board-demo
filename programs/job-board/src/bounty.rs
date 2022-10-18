use anchor_lang::prelude::*;

pub const BOUNTY_SIZE: usize = 113;
pub const BOUNTY_PREFIX: &str = "bounty";

#[account]
pub struct Bounty {
    /// Who has provided the bounty
    pub sponsor: Pubkey,
    /// Who can claim bounty
    pub recipient: Pubkey,
    /// Which token type is being offered (also needed to get escrow ATA)
    pub mint: Pubkey,
    /// How much is offered
    pub amount: u64,
    /// Unix timestamp
    pub slot: u64,
    /// Set to true when employee
    pub accepted: bool,
}
