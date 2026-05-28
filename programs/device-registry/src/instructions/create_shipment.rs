use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;

pub fn handler(
    ctx: Context<CreateShipment>,
    nonce: [u8; 32],
    manifest_commitment: [u8; 32],
) -> Result<()> {
    let shipment = &mut ctx.accounts.shipment;

    shipment.authority = ctx.accounts.authority.key();
    shipment.nonce = nonce;
    shipment.manifest_commitment = manifest_commitment;
    shipment.proof_count = 0;
    shipment.last_commitment = [0u8; 32];
    shipment.closed = false;
    shipment.bump = ctx.bumps.shipment;

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
