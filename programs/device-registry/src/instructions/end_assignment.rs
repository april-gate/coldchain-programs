use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::*;

pub fn handler(ctx: Context<EndAssignment>) -> Result<()> {
    let device = &mut ctx.accounts.device;
    let assignment = &mut ctx.accounts.assignment;
    let clock = Clock::get()?;

    require!(
        device.current_assignment == assignment.key(),
        ColdchainError::AssignmentMismatch
    );
    require!(
        assignment.ended_at == 0,
        ColdchainError::AssignmentAlreadyEnded
    );

    assignment.ended_at = clock.unix_timestamp;
    device.current_assignment = Pubkey::default();

    emit!(AssignmentEnded {
        device: device.key(),
        shipment: assignment.shipment,
        assignment: assignment.key(),
        ended_at: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct EndAssignment<'info> {
    #[account(mut, has_one = authority)]
    pub device: Account<'info, DeviceRegistry>,

    #[account(mut)]
    pub assignment: Account<'info, DeviceAssignment>,

    pub authority: Signer<'info>,
}
