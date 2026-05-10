use anchor_lang::prelude::*;
use crate::state::{DeviceId, ShipmentStatus};

#[event]
pub struct DeviceRegistered {
    pub device: Pubkey,
    pub device_id: DeviceId,
    pub authority: Pubkey,
}

#[event]
pub struct ShipmentCreated {
    pub shipment: Pubkey,
    pub authority: Pubkey,
    pub nonce: u64,
    pub created_at: i64,
}

#[event]
pub struct ShipmentStatusChanged {
    pub shipment: Pubkey,
    pub from: ShipmentStatus,
    pub to: ShipmentStatus,
    pub changed_at: i64,
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

#[event]
pub struct ProofSubmitted {
    pub assignment: Pubkey,
    pub proof: Pubkey,
    pub sequence: u32,
    pub commitment: [u8; 32],
    pub submitted_at: i64,
}
