use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::*;

pub fn handler(ctx: Context<SubmitProof>, commitment: [u8; 32]) -> Result<()> {
    let assignment = &mut ctx.accounts.assignment;
    let shipment = &mut ctx.accounts.shipment;

    require!(!shipment.closed, ColdchainError::ShipmentClosed);

    let sequence = shipment.proof_count;

    shipment.proof_count = shipment
        .proof_count
        .checked_add(1)
        .ok_or(ColdchainError::ArithmeticOverflow)?;
    shipment.last_commitment = commitment;

    // Per-assignment counter maintained for off-chain accounting.
    assignment.proof_count = assignment
        .proof_count
        .checked_add(1)
        .ok_or(ColdchainError::ArithmeticOverflow)?;

    emit!(ProofSubmitted {
        shipment: shipment.key(),
        assignment: assignment.key(),
        device: ctx.accounts.device.key(),
        sequence,
        commitment,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SubmitProof<'info> {
    #[account(
        mut,
        constraint = assignment.shipment == shipment.key() @ ColdchainError::AssignmentMismatch,
        constraint = assignment.ended_at == 0 @ ColdchainError::AssignmentEnded,
        constraint = assignment.device == device.key() @ ColdchainError::AssignmentDeviceMismatch
    )]
    pub assignment: Account<'info, DeviceAssignment>,

    #[account(mut)]
    pub shipment: Account<'info, Shipment>,

    /// The registered device this dispatch is submitted under. Its recorded
    /// authority must be the transaction signer (hardware-rooted identity).
    pub device: Account<'info, DeviceRegistry>,

    /// Must be the device's recorded authority key. This is the
    /// hardware-rooted identity check: only the registered device's owner
    /// can submit dispatches under its assignment.
    #[account(
        constraint = submitter.key() == device.authority @ ColdchainError::UnauthorizedSubmitter
    )]
    pub submitter: Signer<'info>,
}
