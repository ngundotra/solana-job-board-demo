use anchor_lang::prelude::*;

#[account]
pub struct AvailabilityInfo {
    pub key: Pubkey,
    pub leaf_index: u32,
    pub bounty: Pubkey,
}
