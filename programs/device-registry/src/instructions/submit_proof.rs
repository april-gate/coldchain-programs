use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::*;

pub fn handler(ctx: Context<SubmitProof>, commitment: [u8; 32]) -> Result<()> {
    let assignment = &mut ctx.accounts.assignment;
    let shipment = &mut ctx.accounts.shipment;
    let proof = &mut ctx.accounts.proof;
    let clock = Clock::get()?;

    // MVP: store commitment only. Production: verify Groth16 proof here
    // using public inputs that bind device_id and shipment_id into the circuit.

    let sequence = assignment.proof_count;

    proof.assignment = assignment.key();
    proof.sequence = sequence;
    proof.commitment = commitment;
    proof.submitter = ctx.accounts.submitter.key();
    proof.submitted_at = clock.unix_timestamp;
    proof.bump = ctx.bumps.proof;

    assignment.proof_count = assignment
        .proof_count
        .checked_add(1)
        .ok_or(ColdchainError::ArithmeticOverflow)?;
    shipment.proof_count = shipment
        .proof_count
        .checked_add(1)
        .ok_or(ColdchainError::ArithmeticOverflow)?;

    emit!(ProofSubmitted {
        assignment: assignment.key(),
        proof: proof.key(),
        sequence,
        commitment,
        submitted_at: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SubmitProof<'info> {
    #[account(mut)]
    pub assignment: Account<'info, DeviceAssignment>,

    #[account(
        mut,
        constraint = assignment.shipment == shipment.key() @ ColdchainError::AssignmentMismatch
    )]
    pub shipment: Account<'info, Shipment>,

    #[account(
        init,
        payer = submitter,
        space = 8 + Proof::INIT_SPACE,
        seeds = [
            b"proof",
            assignment.key().as_ref(),
            &assignment.proof_count.to_le_bytes()
        ],
        bump
    )]
    pub proof: Account<'info, Proof>,

    #[account(mut)]
    pub submitter: Signer<'info>,

    pub system_program: Program<'info, System>,
}
