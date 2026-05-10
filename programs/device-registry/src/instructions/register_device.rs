use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;

pub fn handler(ctx: Context<RegisterDevice>, device_id: DeviceId, pubkey: [u8; 33]) -> Result<()> {
    let device = &mut ctx.accounts.device;
    device.authority = ctx.accounts.authority.key();
    device.device_id = device_id;
    device.pubkey = pubkey;
    device.assignment_count = 0;
    device.current_assignment = Pubkey::default();
    device.bump = ctx.bumps.device;

    emit!(DeviceRegistered {
        device: device.key(),
        device_id,
        authority: device.authority,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(device_id: DeviceId)]
pub struct RegisterDevice<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + DeviceRegistry::INIT_SPACE,
        seeds = [b"device", device_id.as_ref()],
        bump
    )]
    pub device: Account<'info, DeviceRegistry>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
