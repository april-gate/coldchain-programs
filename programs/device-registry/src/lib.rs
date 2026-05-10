use anchor_lang::prelude::*;

pub mod state;
pub mod errors;
pub mod events;
pub mod instructions;

// `pub use` (not just `use`) re-exports instruction items at the crate root,
// which is where the #[program] macro expects to find Accounts structs and
// their auto-generated __client_accounts_* / __cpi_client_accounts_* modules.
pub use instructions::*;
use state::{DeviceId, ShipmentStatus};

declare_id!("APRu6WGxe1NC4X2FrcLpujRRtqLNfMTSt6fYp5wQZVtP");

#[program]
pub mod device_registry {
    use super::*;

    /// Register a new device. Creates an immutable PDA keyed by device_id.
    pub fn register_device(
        ctx: Context<RegisterDevice>,
        device_id: DeviceId,
        pubkey: [u8; 33],
    ) -> Result<()> {
        instructions::register_device::handler(ctx, device_id, pubkey)
    }

    /// Create a new shipment owned by the caller. Status starts as Created.
    pub fn create_shipment(ctx: Context<CreateShipment>, nonce: u64) -> Result<()> {
        instructions::create_shipment::handler(ctx, nonce)
    }

    /// Transition a shipment's status. Allowed transitions:
    /// Created → InTransit → Delivered → Closed.
    pub fn update_shipment_status(
        ctx: Context<UpdateShipmentStatus>,
        new_status: ShipmentStatus,
    ) -> Result<()> {
        instructions::update_shipment_status::handler(ctx, new_status)
    }

    /// Assign a device to a shipment. Creates an immutable DeviceAssignment
    /// PDA, preserving the device's full assignment history.
    pub fn assign_device(ctx: Context<AssignDevice>) -> Result<()> {
        instructions::assign_device::handler(ctx)
    }

    /// Mark the device's current assignment as ended.
    pub fn end_assignment(ctx: Context<EndAssignment>) -> Result<()> {
        instructions::end_assignment::handler(ctx)
    }

    /// Submit a ZK proof commitment against an active assignment.
    /// MVP stores commitment only; production verifies Groth16 on-chain.
    pub fn submit_proof(ctx: Context<SubmitProof>, commitment: [u8; 32]) -> Result<()> {
        instructions::submit_proof::handler(ctx, commitment)
    }
}