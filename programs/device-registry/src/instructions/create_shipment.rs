use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;

pub fn handler(ctx: Context<CreateShipment>, nonce: u64) -> Result<()> {
    let shipment = &mut ctx.accounts.shipment;
    let clock = Clock::get()?;

    shipment.authority = ctx.accounts.authority.key();
    shipment.nonce = nonce;
    shipment.status = ShipmentStatus::Created;
    shipment.created_at = clock.unix_timestamp;
    shipment.proof_count = 0;
    shipment.bump = ctx.bumps.shipment;

    emit!(ShipmentCreated {
        shipment: shipment.key(),
        authority: shipment.authority,
        nonce,
        created_at: shipment.created_at,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct CreateShipment<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Shipment::INIT_SPACE,
        seeds = [
            b"shipment",
            authority.key().as_ref(),
            &nonce.to_le_bytes()
        ],
        bump
    )]
    pub shipment: Account<'info, Shipment>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
