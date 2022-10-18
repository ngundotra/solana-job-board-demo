use anchor_lang::prelude::*;
use anchor_lang::solana_program::{keccak::hashv, system_program};
// use anchor_spl::associated_token::{create, Create};
use anchor_spl::token::{
    close_account, transfer, CloseAccount, Mint, Token, TokenAccount, Transfer,
};
use spl_account_compression::{
    self, state::CONCURRENT_MERKLE_TREE_HEADER_SIZE_V1, zero_copy::ZeroCopy, Node, Wrapper,
};
use spl_concurrent_merkle_tree::concurrent_merkle_tree::ConcurrentMerkleTree;

mod availability_info;
mod bounty;
mod error;

use crate::availability_info::*;
use crate::bounty::*;
use crate::error::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[derive(Clone)]
pub struct AccountCompressionProgram {}
impl anchor_lang::Id for AccountCompressionProgram {
    fn id() -> Pubkey {
        spl_account_compression::id()
    }
}

pub const GLOBAL_AUTH_PREFIX: &str = "global_auth";
pub const AVAILABILITY_PREFIX: &str = "available";
pub const AVAILABILITY_INFO_SIZE: usize = 36;
pub const AVAILABILITY_TREE_BUFFER_SIZE: usize = 64;
pub const AVAILABILITY_TREE_DEPTH: usize = 20;

#[derive(Accounts)]
pub struct InitializeGlobals<'info> {
    /// CHECK: Checked in Account Compression
    oracle_tree: AccountInfo<'info>,
    /// CHECK: Checked in Account Compression
    available_tree: AccountInfo<'info>,
    /// CHECK: Assert that the owner initializes
    whitelisted_key: Signer<'info>,
    /// CHECK: Global Auth used to guarantee only this program can write to the `oracle_tree`
    #[account(
        seeds=[&GLOBAL_AUTH_PREFIX.as_bytes()],
        bump
    )]
    global_auth: AccountInfo<'info>,
    spl_account_compression: Program<'info, AccountCompressionProgram>,
    spl_noop: Program<'info, Wrapper>,
}

#[derive(Accounts)]
pub struct UpdateJobBoard<'info> {
    /// CHECK: Checked in Account Compression
    oracle_tree: AccountInfo<'info>,
    /// CHECK: Global Auth used to guarantee only this program can write to the `oracle_tree`
    #[account(
        seeds=[GLOBAL_AUTH_PREFIX.as_bytes()],
        bump
    )]
    global_auth: AccountInfo<'info>,
    spl_account_compression: Program<'info, AccountCompressionProgram>,
    spl_noop: Program<'info, Wrapper>,
}

fn create_available_leaf(key: &Pubkey) -> Node {
    hashv(&[&key.as_ref(), &[0x1]]).to_bytes()
}

fn create_unavailable_leaf(key: &Pubkey) -> Node {
    hashv(&[&key.as_ref(), &[0x0]]).to_bytes()
}

macro_rules! empty_key {
    () => {
        Pubkey::new_from_array([0; 32])
    };
}

#[derive(Accounts)]
pub struct RegisterMyAvailability<'info> {
    #[account(mut)]
    payer: Signer<'info>,
    myself: Signer<'info>,
    #[account(
        init,
        payer=payer,
        space=AVAILABILITY_INFO_SIZE + 8,
        seeds=[AVAILABILITY_PREFIX.as_bytes(), &myself.key.to_bytes()],
        bump
    )]
    my_availability_info: Account<'info, AvailabilityInfo>,
    /// CHECK: Checked in Account Compression
    #[account(mut)]
    available_tree: AccountInfo<'info>,
    /// CHECK: Global Auth used to guarantee only this program can write to the `oracle_tree`
    #[account(
        seeds=[GLOBAL_AUTH_PREFIX.as_bytes()],
        bump
    )]
    global_auth: AccountInfo<'info>,
    spl_account_compression: Program<'info, AccountCompressionProgram>,
    spl_noop: Program<'info, Wrapper>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ChangeMyAvailability<'info> {
    myself: Signer<'info>,
    #[account(
        mut,
        seeds=[AVAILABILITY_PREFIX.as_bytes(), &myself.key.to_bytes()],
        bump
    )]
    my_availability_info: Account<'info, AvailabilityInfo>,
    /// CHECK: Checked in Account Compression
    available_tree: AccountInfo<'info>,
    /// CHECK: Global Auth used to guarantee only this program can write to the `oracle_tree`
    #[account(
        seeds=[GLOBAL_AUTH_PREFIX.as_bytes()],
        bump
    )]
    global_auth: AccountInfo<'info>,
    spl_account_compression: Program<'info, AccountCompressionProgram>,
    spl_noop: Program<'info, Wrapper>,
}

fn init_tree<'info>(
    authority_bump: u8,
    max_depth: u32,
    max_buffer_size: u32,
    merkle_tree: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    spl_noop: AccountInfo<'info>,
    spl_account_compression: &Program<'info, AccountCompressionProgram>,
) -> Result<()> {
    let seeds = &[GLOBAL_AUTH_PREFIX.as_bytes(), &[authority_bump]];
    let signer_seeds = &[&seeds[..]];
    let cpi_ctx = CpiContext::new_with_signer(
        spl_account_compression.to_account_info(),
        spl_account_compression::cpi::accounts::Initialize {
            merkle_tree,
            authority,
            log_wrapper: spl_noop,
        },
        signer_seeds,
    );
    spl_account_compression::cpi::init_empty_merkle_tree(cpi_ctx, max_depth, max_buffer_size)?;
    Ok(())
}

fn replace_leaf<'info>(
    authority_bump: u8,
    index: u32,
    new_leaf: Node,
    prev_leaf: Node,
    root: Node,
    merkle_tree: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    spl_noop: AccountInfo<'info>,
    spl_account_compression: &Program<'info, AccountCompressionProgram>,
) -> Result<()> {
    let seeds = &[GLOBAL_AUTH_PREFIX.as_bytes(), &[authority_bump]];
    let signer_seeds = &[&seeds[..]];
    let cpi_ctx = CpiContext::new_with_signer(
        spl_account_compression.to_account_info(),
        spl_account_compression::cpi::accounts::Modify {
            merkle_tree,
            authority,
            log_wrapper: spl_noop,
        },
        signer_seeds,
    );
    spl_account_compression::cpi::replace_leaf(cpi_ctx, root, prev_leaf, new_leaf, index)?;
    Ok(())
}

fn append_leaf<'info>(
    authority_bump: u8,
    new_leaf: Node,
    merkle_tree: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    spl_noop: AccountInfo<'info>,
    spl_account_compression: &Program<'info, AccountCompressionProgram>,
) -> Result<()> {
    let seeds = &[GLOBAL_AUTH_PREFIX.as_bytes(), &[authority_bump]];
    let signer_seeds = &[&seeds[..]];
    let cpi_ctx = CpiContext::new_with_signer(
        spl_account_compression.to_account_info(),
        spl_account_compression::cpi::accounts::Modify {
            merkle_tree,
            authority,
            log_wrapper: spl_noop,
        },
        signer_seeds,
    );
    spl_account_compression::cpi::append(cpi_ctx, new_leaf)?;
    Ok(())
}

#[derive(Accounts)]
pub struct CreateBounty<'info> {
    /// CHECK: putting up the funds
    #[account(mut)]
    pub sponsor: Signer<'info>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub sponsor_token_account: Account<'info, TokenAccount>,
    /// CHECK: the account to recieve the funds
    pub recipient: AccountInfo<'info>,
    #[account(
        init,
        payer = sponsor,
        space = 8 + BOUNTY_SIZE,
        seeds=[&BOUNTY_PREFIX.as_bytes(), sponsor.key.as_ref(), recipient.key.as_ref()],
        bump
    )]
    pub bounty: Account<'info, Bounty>,
    /// CHECK: to be initialized by SPL token
    #[account(mut)]
    pub bounty_ata: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub spl_token: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CloseBounty<'info> {
    /// CHECK: putting up the funds
    #[account(mut)]
    pub sponsor: Signer<'info>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub sponsor_token_account: Account<'info, TokenAccount>,
    /// CHECK: who the bounty was posted for
    pub recipient: AccountInfo<'info>,
    #[account(
        mut,
        seeds=[&BOUNTY_PREFIX.as_bytes(), sponsor.key.as_ref(), recipient.key.as_ref()],
        bump
    )]
    pub bounty: Account<'info, Bounty>,
    #[account(mut)]
    pub bounty_ata: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub spl_token: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AcceptBounty<'info> {
    /// CHECK: putting up the funds
    pub sponsor: AccountInfo<'info>,
    pub token_mint: Account<'info, Mint>,
    pub recipient: Signer<'info>,
    #[account(
        mut,
        seeds=[AVAILABILITY_PREFIX.as_bytes(), &recipient.key.to_bytes()],
        bump
    )]
    pub recipient_info: Account<'info, AvailabilityInfo>,
    #[account(mut)]
    pub recipient_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds=[&BOUNTY_PREFIX.as_bytes(), sponsor.key.as_ref(), recipient.key.as_ref()],
        bump
    )]
    pub bounty: Account<'info, Bounty>,
    #[account(mut)]
    pub bounty_ata: Account<'info, TokenAccount>,
    pub spl_token: Program<'info, Token>,
}

#[program]
pub mod job_board {
    use super::*;

    pub fn initialize_globals(ctx: Context<InitializeGlobals>) -> Result<()> {
        // TODO(@ngundotra): Check whitelisted signer
        let signer_bump = *ctx.bumps.get("global_auth").unwrap();

        let oracle_max_depth = 20;
        let oracle_max_buffer_size = 64;
        init_tree(
            signer_bump,
            oracle_max_depth,
            oracle_max_buffer_size,
            ctx.accounts.oracle_tree.to_account_info(),
            ctx.accounts.global_auth.to_account_info(),
            ctx.accounts.spl_noop.to_account_info(),
            &ctx.accounts.spl_account_compression,
        )?;

        init_tree(
            signer_bump,
            AVAILABILITY_TREE_DEPTH as u32,
            AVAILABILITY_TREE_BUFFER_SIZE as u32,
            ctx.accounts.available_tree.to_account_info(),
            ctx.accounts.global_auth.to_account_info(),
            ctx.accounts.spl_noop.to_account_info(),
            &ctx.accounts.spl_account_compression,
        )?;

        Ok(())
    }

    /// Update on-chain oracle
    pub fn update_job_board(
        ctx: Context<UpdateJobBoard>,
        leaf_index: u32,
        new_leaf: [u8; 32],
        prev_leaf: [u8; 32],
        root: [u8; 32],
    ) -> Result<()> {
        let signer_bump = *ctx.bumps.get("global_auth").unwrap();
        replace_leaf(
            signer_bump,
            leaf_index,
            new_leaf,
            prev_leaf,
            root,
            ctx.accounts.oracle_tree.to_account_info(),
            ctx.accounts.global_auth.to_account_info(),
            ctx.accounts.spl_noop.to_account_info(),
            &ctx.accounts.spl_account_compression,
        )?;

        Ok(())
    }

    /// Allow composing programs to read data in from here
    /// as an oracle
    /// Technically not necessary... but whatevaaaa
    // pub fn verify_credentials(

    // ) -> Result<()> {

    //     Ok(())
    // }

    pub fn register_availability(ctx: Context<RegisterMyAvailability>) -> Result<()> {
        let myself = &ctx.accounts.myself;
        let my_info = &mut ctx.accounts.my_availability_info;
        let availability_tree = &ctx.accounts.available_tree;
        my_info.key = *myself.key;
        my_info.bounty = empty_key!();

        // deserialize the tree with zero copy
        let mut merkle_tree_bytes = availability_tree.try_borrow_mut_data()?;
        let (_, bytes) = merkle_tree_bytes.split_at_mut(CONCURRENT_MERKLE_TREE_HEADER_SIZE_V1);
        let tree = ConcurrentMerkleTree::<AVAILABILITY_TREE_DEPTH, AVAILABILITY_TREE_BUFFER_SIZE>::load_mut_bytes(bytes)?;

        // We will append the pubkey's information into this index
        my_info.leaf_index = tree.rightmost_proof.index;

        // Append info
        let signer_bump = *ctx.bumps.get("global_auth").unwrap();
        append_leaf(
            signer_bump,
            create_available_leaf(&myself.key),
            ctx.accounts.available_tree.to_account_info(),
            ctx.accounts.global_auth.to_account_info(),
            ctx.accounts.spl_noop.to_account_info(),
            &ctx.accounts.spl_account_compression,
        )?;

        Ok(())
    }

    /// Will fail if already available
    pub fn set_myself_available(ctx: Context<ChangeMyAvailability>, root: [u8; 32]) -> Result<()> {
        let my_info = &mut ctx.accounts.my_availability_info;

        // Read from remaining accounts
        if my_info.bounty != system_program::ID {
            if ctx.remaining_accounts.len() < 1 {
                return Err(JobBoardError::BountyAccountMissing.into());
            }
            let bounty_account = &ctx.remaining_accounts[0];

            // check bounty_account
            let bounty_data = bounty_account.try_borrow_mut_data()?;
            let mut bounty = Bounty::try_from_slice(&bounty_data).unwrap();

            let clock = Clock::get()?;
            // If bounty active, err
            if bounty.accepted && bounty.slot > clock.slot {
                return Err(JobBoardError::BountyDelistTimeoutStillActive.into());
            }
            // otherwise, remove ties to bounty
            if bounty.accepted && bounty.slot <= clock.slot {
                bounty.accepted = false;
                my_info.bounty = empty_key!();
            }
        }

        let prev_leaf = create_unavailable_leaf(&ctx.accounts.myself.key);
        let new_leaf = create_available_leaf(&ctx.accounts.myself.key);

        let signer_bump = *ctx.bumps.get("global_auth").unwrap();
        replace_leaf(
            signer_bump,
            ctx.accounts.my_availability_info.leaf_index,
            new_leaf,
            prev_leaf,
            root,
            ctx.accounts.available_tree.to_account_info(),
            ctx.accounts.global_auth.to_account_info(),
            ctx.accounts.spl_noop.to_account_info(),
            &ctx.accounts.spl_account_compression,
        )?;
        Ok(())
    }

    /// Will fail if already unavailable
    pub fn set_myself_unavailable(
        ctx: Context<ChangeMyAvailability>,
        root: [u8; 32],
    ) -> Result<()> {
        let prev_leaf = create_available_leaf(&ctx.accounts.myself.key);
        let new_leaf = create_unavailable_leaf(&ctx.accounts.myself.key);

        let signer_bump = *ctx.bumps.get("global_auth").unwrap();
        replace_leaf(
            signer_bump,
            ctx.accounts.my_availability_info.leaf_index,
            new_leaf,
            prev_leaf,
            root,
            ctx.accounts.available_tree.to_account_info(),
            ctx.accounts.global_auth.to_account_info(),
            ctx.accounts.spl_noop.to_account_info(),
            &ctx.accounts.spl_account_compression,
        )?;
        Ok(())
    }

    pub fn create_bounty(ctx: Context<CreateBounty>, slot: u64, amount: u64) -> Result<()> {
        let sponsor = &ctx.accounts.sponsor;
        let sponsor_token_account = &ctx.accounts.sponsor_token_account;
        let recipient = &ctx.accounts.recipient;
        let mint = &ctx.accounts.token_mint;
        let bounty_ata = &ctx.accounts.bounty_ata;
        let spl_token = &ctx.accounts.spl_token;

        // Init bounty
        let bounty = &mut ctx.accounts.bounty;
        bounty.set_inner(Bounty {
            sponsor: *sponsor.key,
            recipient: *recipient.key,
            mint: mint.key(),
            amount,
            slot,
            accepted: false,
        });

        // Transfer from the sponsor to ata
        let transfer_ctx = CpiContext::new(
            spl_token.to_account_info(),
            Transfer {
                from: sponsor_token_account.to_account_info(),
                to: bounty_ata.to_account_info(),
                authority: sponsor.to_account_info(),
            },
        );
        transfer(transfer_ctx, amount)?;

        Ok(())
    }

    pub fn accept_bounty(ctx: Context<AcceptBounty>) -> Result<()> {
        let bounty = &mut ctx.accounts.bounty;
        let recipient_info = &mut ctx.accounts.recipient_info;
        let bounty_ata = &ctx.accounts.bounty_ata;
        let recipient = &ctx.accounts.recipient;
        let recipient_token_account = &ctx.accounts.recipient_token_account;
        let spl_token = &ctx.accounts.spl_token;

        // set bounty key to match
        bounty.accepted = true;

        // set bounty accepted to active
        recipient_info.bounty = bounty.key();

        // transfer funds
        let bump = *ctx.bumps.get("bounty").unwrap();
        transfer(
            CpiContext::new_with_signer(
                spl_token.to_account_info(),
                Transfer {
                    from: bounty_ata.to_account_info(),
                    to: recipient_token_account.to_account_info(),
                    authority: bounty.to_account_info(),
                },
                &[&[
                    &AVAILABILITY_PREFIX.as_bytes(),
                    &recipient.key.to_bytes(),
                    &[bump],
                ]],
            ),
            bounty_ata.amount,
        )?;

        Ok(())
    }

    pub fn close_bounty(ctx: Context<CloseBounty>) -> Result<()> {
        let bounty = &mut ctx.accounts.bounty;
        let bounty_ata = &ctx.accounts.bounty_ata;
        let recipient = &ctx.accounts.recipient;
        let sponsor = &ctx.accounts.sponsor;
        let sponsor_token_account = &ctx.accounts.sponsor_token_account;
        let spl_token = &ctx.accounts.spl_token;

        // Transfer tokens out
        let bump_seed = *ctx.bumps.get("bounty").unwrap();
        transfer(
            CpiContext::new_with_signer(
                spl_token.to_account_info(),
                Transfer {
                    from: bounty_ata.to_account_info(),
                    to: sponsor_token_account.to_account_info(),
                    authority: bounty.to_account_info(),
                },
                &[&[
                    &BOUNTY_PREFIX.as_bytes(),
                    &sponsor.key.as_ref(),
                    &recipient.key.as_ref(),
                    &[bump_seed],
                ]],
            ),
            bounty_ata.amount,
        )?;

        // Close bounty ata
        close_account(CpiContext::new_with_signer(
            spl_token.to_account_info(),
            CloseAccount {
                account: bounty_ata.to_account_info(),
                destination: sponsor.to_account_info(),
                authority: bounty.to_account_info(),
            },
            &[&[
                &BOUNTY_PREFIX.as_bytes(),
                &sponsor.key.as_ref(),
                &recipient.key.as_ref(),
                &[bump_seed],
            ]],
        ))?;

        // Close bounty
        let bounty_ai = bounty.to_account_info();
        let dest_lamports = sponsor.lamports();
        **sponsor.lamports.borrow_mut() = bounty_ai.lamports().checked_add(dest_lamports).unwrap();
        **bounty_ai.lamports.borrow_mut() = 0;

        let mut bounty_data = bounty_ai.try_borrow_mut_data()?;
        bounty_data.fill(0);

        Ok(())
    }
}
