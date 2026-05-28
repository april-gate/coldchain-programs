use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::*;

pub fn handler(ctx: Context<CloseShipment>) -> Result<()> {
    let shipment = &mut ctx.accounts.shipment;

    require!(!shipment.closed, ColdchainError::ShipmentClosed);

    shipment.closed = true;

    emit!(ShipmentClosed {
        shipment: shipment.key(),
        proof_count: shipment.proof_count,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct CloseShipment<'info> {
    /// Operator-only, one-way. The `has_one = authority` constraint ensures
    /// only the wallet that founded the shipment can dissolve it.
    #[account(mut, has_one = authority)]
    pub shipment: Account<'info, Shipment>,

    pub authority: Signer<'info>,
}
