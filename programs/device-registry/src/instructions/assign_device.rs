use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::*;

pub fn handler(ctx: Context<AssignDevice>) -> Result<()> {
    let device = &mut ctx.accounts.device;
    let shipment = &ctx.accounts.shipment;
    let assignment = &mut ctx.accounts.assignment;
    let clock = Clock::get()?;

    require!(
        device.current_assignment == Pubkey::default(),
        ColdchainError::DeviceAlreadyAssigned
    );

    require!(
        matches!(shipment.status, ShipmentStatus::Created | ShipmentStatus::InTransit),
        ColdchainError::ShipmentNotAcceptingDevices
    );

    let sequence = device.assignment_count;

    assignment.device = device.key();
    assignment.shipment = shipment.key();
    assignment.sequence = sequence;
    assignment.authority = ctx.accounts.authority.key();
    assignment.assigned_at = clock.unix_timestamp;
    assignment.ended_at = 0;
    assignment.proof_count = 0;
    assignment.bump = ctx.bumps.assignment;

    device.current_assignment = assignment.key();
    device.assignment_count = device
        .assignment_count
        .checked_add(1)
        .ok_or(ColdchainError::ArithmeticOverflow)?;

    emit!(DeviceAssigned {
        device: device.key(),
        shipment: shipment.key(),
        assignment: assignment.key(),
        sequence,
        assigned_at: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct AssignDevice<'info> {
    #[account(mut, has_one = authority)]
    pub device: Account<'info, DeviceRegistry>,

    pub shipment: Account<'info, Shipment>,

    #[account(
        init,
        payer = authority,
        space = 8 + DeviceAssignment::INIT_SPACE,
        seeds = [
            b"assignment",
            device.key().as_ref(),
            &device.assignment_count.to_le_bytes()
        ],
        bump
    )]
    pub assignment: Account<'info, DeviceAssignment>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
