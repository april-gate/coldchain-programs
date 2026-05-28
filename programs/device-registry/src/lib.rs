use anchor_lang::prelude::*;

pub mod state;
pub mod errors;
pub mod events;
pub mod instructions;

// `pub use` (not just `use`) re-exports instruction items at the crate root,
// which is where the #[program] macro expects to find Accounts structs and
// their auto-generated __client_accounts_* / __cpi_client_accounts_* modules.
pub use instructions::*;
use state::DeviceId;

declare_id!("APRBVwwJJeStD5wShyg4HivneDYj4TCPYKtSFX5F4jez");

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

    /// Found a shipment subnet. `nonce` is a 32-byte unguessable identifier;
    /// `manifest_commitment` binds the off-chain shipment manifest.
    pub fn create_shipment(
        ctx: Context<CreateShipment>,
        nonce: [u8; 32],
        manifest_commitment: [u8; 32],
    ) -> Result<()> {
        instructions::create_shipment::handler(ctx, nonce, manifest_commitment)
    }

    /// Assign a device to a shipment. Creates an immutable DeviceAssignment
    /// PDA, preserving the device's full assignment history. Rejected if the
    /// shipment is closed.
    pub fn assign_device(ctx: Context<AssignDevice>) -> Result<()> {
        instructions::assign_device::handler(ctx)
    }

    /// Mark the device's current assignment as ended. Permitted regardless of
    /// shipment closed state, so devices can be freed for reuse.
    pub fn end_assignment(ctx: Context<EndAssignment>) -> Result<()> {
        instructions::end_assignment::handler(ctx)
    }

    /// Submit a consensus dispatch commitment against an active assignment.
    /// The submitter must be the device's recorded hardware-rooted authority.
    /// No account is created; the Shipment PDA is updated and the dispatch is
    /// emitted to the transaction log.
    pub fn submit_proof(ctx: Context<SubmitProof>, commitment: [u8; 32]) -> Result<()> {
        instructions::submit_proof::handler(ctx, commitment)
    }

    /// Dissolve the shipment subnet. Operator-only, one-way. After close, no
    /// new dispatches or assignments are accepted.
    pub fn close_shipment(ctx: Context<CloseShipment>) -> Result<()> {
        instructions::close_shipment::handler(ctx)
    }
}
