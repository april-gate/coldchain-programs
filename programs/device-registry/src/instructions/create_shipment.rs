use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use crate::state::*;
use crate::events::*;

pub fn handler(
    ctx: Context<CreateShipment>,
    nonce: [u8; 32],
    manifest_commitment: [u8; 32],
) -> Result<()> {
    let created_at = Clock::get()?.unix_timestamp;
    let shipment = &mut ctx.accounts.shipment;

    shipment.authority = ctx.accounts.authority.key();
    shipment.nonce = nonce;
    shipment.manifest_commitment = manifest_commitment;
    shipment.proof_count = 0;
    shipment.last_commitment = [0u8; 32];
    shipment.created_at = created_at;
    shipment.closed = false;
    shipment.bump = ctx.bumps.shipment;
    shipment.attestations = Vec::new();

    // Seed the hash chain with a domain-separated genesis hash bound to the
    // shipment's identity (PDA address + creation timestamp). Binding genesis
    // to identity — rather than using all-zeros — guarantees the exportable
    // chain hash is unique per shipment from the very first proof, so two
    // shipments with identical proof histories can never be spuriously
    // identified as equivalent during off-chain audit.
    let genesis_hash = hashv(&[
        Shipment::GENESIS_DOMAIN,
        shipment.key().as_ref(),
        &created_at.to_le_bytes()[..],
    ]);
    shipment.chain_hash = genesis_hash.to_bytes();

    emit!(ShipmentCreated {
        shipment: shipment.key(),
        authority: shipment.authority,
        manifest_commitment,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(nonce: [u8; 32])]
pub struct CreateShipment<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Shipment::INIT_SPACE,
        seeds = [
            b"shipment",
            authority.key().as_ref(),
            nonce.as_ref()
        ],
        bump
    )]
    pub shipment: Account<'info, Shipment>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
