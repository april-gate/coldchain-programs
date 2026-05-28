use anchor_lang::prelude::*;
use crate::state::DeviceId;

#[event]
pub struct DeviceRegistered {
    pub device: Pubkey,
    pub device_id: DeviceId,
    pub authority: Pubkey,
}

/// Emitted by `create_shipment`. Marks subnet formation.
#[event]
pub struct ShipmentCreated {
    pub shipment: Pubkey,
    pub authority: Pubkey,
    pub manifest_commitment: [u8; 32],
}

/// Emitted by `close_shipment`. Marks subnet dissolution. Final proof_count
/// is captured here so an auditor reading just the close transaction's logs
/// knows the total dispatch count without also fetching the Shipment account.
#[event]
pub struct ShipmentClosed {
    pub shipment: Pubkey,
    pub proof_count: u32,
}

#[event]
pub struct DeviceAssigned {
    pub device: Pubkey,
    pub shipment: Pubkey,
    pub assignment: Pubkey,
    pub sequence: u32,
    pub assigned_at: i64,
}

#[event]
pub struct AssignmentEnded {
    pub device: Pubkey,
    pub shipment: Pubkey,
    pub assignment: Pubkey,
    pub ended_at: i64,
}

/// Emitted by `submit_proof`. The per-round audit record — a subnet member's
/// consensus dispatch — lives in the transaction log permanently. Solana
/// blockTime (via getTransaction) is the authoritative chain-witnessed
/// timestamp; not duplicated here.
#[event]
pub struct ProofSubmitted {
    pub shipment: Pubkey,
    pub assignment: Pubkey,
    pub device: Pubkey,
    /// Chain-assigned sequence within this shipment. 0 for the first proof.
    pub sequence: u32,
    /// 32-byte hash binding the off-chain consensus dispatch content.
    pub commitment: [u8; 32],
}
