use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::*;

pub fn handler(ctx: Context<UpdateShipmentStatus>, new_status: ShipmentStatus) -> Result<()> {
    let shipment = &mut ctx.accounts.shipment;
    let from = shipment.status;
    let clock = Clock::get()?;

    require!(
        Shipment::can_transition(from, new_status),
        ColdchainError::InvalidStatusTransition
    );

    shipment.status = new_status;

    emit!(ShipmentStatusChanged {
        shipment: shipment.key(),
        from,
        to: new_status,
        changed_at: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateShipmentStatus<'info> {
    #[account(mut, has_one = authority)]
    pub shipment: Account<'info, Shipment>,

    pub authority: Signer<'info>,
}
