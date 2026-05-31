use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use crate::state::*;
use crate::events::*;
use crate::errors::*;

pub fn handler(ctx: Context<SubmitProof>, commitment: [u8; 32]) -> Result<()> {
    let assignment = &mut ctx.accounts.assignment;
    let shipment = &mut ctx.accounts.shipment;

    require!(!shipment.closed, ColdchainError::ShipmentClosed);

    let sequence = shipment.proof_count;

    // Fold this proof into the integrity hash chain BEFORE incrementing
    // proof_count, so the sequence bound into the hash matches the event's
    // sequence number (0 for the first proof). Input order is fixed:
    //   prev_chain_hash (32) || proof_commitment (32) || sequence_le (4)
    // Folding the sequence in prevents an attacker from reordering proofs and
    // arriving at the same final chain hash.
    let new_chain_hash = hashv(&[
        &shipment.chain_hash[..],
        &commitment[..],
        &sequence.to_le_bytes()[..],
    ])
    .to_bytes();
    shipment.chain_hash = new_chain_hash;

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
        chain_hash_after: new_chain_hash,
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
