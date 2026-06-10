use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as instructions_sysvar;

use crate::errors::*;
use crate::events::*;
use crate::state::*;

pub fn handler(
    ctx: Context<AttestShipmentVerification>,
    chain_hash_at_verification: [u8; 32],
    outcome: VerificationOutcome,
    attestation_signature: [u8; 64],
) -> Result<()> {
    let shipment = &mut ctx.accounts.shipment;

    // 1. A shipment must be dissolved (closed) before it can be attested — the
    //    proof history an attestation pins to is only final after close.
    require!(shipment.closed, ColdchainError::ShipmentNotClosed);

    // 2. Capacity check against the reserved inline storage budget.
    require!(
        shipment.attestations.len() < Shipment::MAX_ATTESTATIONS,
        ColdchainError::AttestationCapacityExceeded
    );

    // 3. Verify the attestation signature.
    //    TODO(signature-verification): The hardened design verifies the
    //    verifier's ed25519 signature over the attestation payload
    //      shipment_pubkey || chain_hash_at_verification || outcome_byte
    //      || verified_at_le_bytes
    //    via a cross-instruction check against the instructions sysvar (the
    //    caller co-submits an ed25519 verify instruction in the same
    //    transaction; this handler confirms its pubkey == verifier.key() and
    //    its message matches the expected payload). The `instructions_sysvar`
    //    account is already part of this instruction's account layout so the
    //    check can be added later WITHOUT changing the layout. For now the
    //    signature is recorded on-chain and verification is deferred to
    //    off-chain consumers, who choose which verifier pubkeys they trust.
    let _ = &ctx.accounts.instructions_sysvar;

    let verified_at = Clock::get()?.unix_timestamp;
    let verifier = ctx.accounts.verifier.key();

    shipment.attestations.push(VerificationAttestation {
        verifier,
        verified_at,
        chain_hash_at_verification,
        outcome,
        attestation_signature,
    });

    emit!(ShipmentAttested {
        shipment: shipment.key(),
        verifier,
        verified_at,
        chain_hash_at_verification,
        outcome,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct AttestShipmentVerification<'info> {
    #[account(
        mut,
        seeds = [
            b"shipment",
            shipment.authority.as_ref(),
            shipment.nonce.as_ref(),
        ],
        bump = shipment.bump,
    )]
    pub shipment: Account<'info, Shipment>,

    /// The verifier — anyone may attest. Authorization at the attestation-trust
    /// level is an off-chain concern (downstream consumers decide which
    /// verifier pubkeys they trust).
    #[account(mut)]
    pub verifier: Signer<'info>,

    /// CHECK: Instructions sysvar, reserved for cross-instruction ed25519
    /// signature verification of the attestation payload (see handler TODO).
    /// Constrained by address so the layout is stable for the hardened path.
    #[account(address = instructions_sysvar::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}
