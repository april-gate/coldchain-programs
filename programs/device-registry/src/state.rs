use anchor_lang::prelude::*;

/// 32-byte device identifier.
///
/// Currently: ATECC608 serial number in bytes [0..9], remaining bytes zero.
/// Reserved for future use: type tag, vendor namespace, identifier extensions.
pub type DeviceId = [u8; 32];

/// Shipment lifecycle states.
///
/// MVP transitions: Created → InTransit → Delivered → Closed.
/// Closed is terminal. Future versions may add Disputed, Cancelled, etc.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum ShipmentStatus {
    Created,
    InTransit,
    Delivered,
    Closed,
}

#[account]
pub struct DeviceRegistry {
    /// Owner authorized to assign this device and submit proofs.
    pub authority: Pubkey,
    /// Hardware-derived device ID (ATECC608 serial in bytes [0..9]).
    pub device_id: DeviceId,
    /// ATECC608 P-256 public key, compressed SEC1 format.
    /// Layout: prefix byte (0x02 or 0x03) + 32-byte X coordinate.
    pub pubkey: [u8; 33],
    /// Total assignments this device has had. Also serves as next sequence.
    pub assignment_count: u32,
    /// Current active assignment PDA, or Pubkey::default() if unassigned.
    pub current_assignment: Pubkey,
    /// PDA bump for re-derivation.
    pub bump: u8,
}

impl DeviceRegistry {
    // authority(32) + device_id(32) + pubkey(33) + assignment_count(4) + current_assignment(32) + bump(1)
    pub const INIT_SPACE: usize = 32 + 32 + 33 + 4 + 32 + 1;
}

#[account]
#[derive(InitSpace)]
pub struct Shipment {
    /// Owner authorized to update status and request device assignments.
    pub authority: Pubkey,
    /// Monotonic shipment counter scoped to authority. Used in PDA seed.
    pub nonce: u64,
    /// Current lifecycle state.
    pub status: ShipmentStatus,
    /// Unix timestamp of shipment creation.
    pub created_at: i64,
    /// Total proofs submitted across all assignments to this shipment.
    pub proof_count: u32,
    /// PDA bump for re-derivation.
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct DeviceAssignment {
    /// Device that was assigned.
    pub device: Pubkey,
    /// Shipment it was assigned to.
    pub shipment: Pubkey,
    /// Sequence number (0 = first assignment for this device).
    pub sequence: u32,
    /// Authority that created the assignment.
    pub authority: Pubkey,
    /// Unix timestamp of assignment.
    pub assigned_at: i64,
    /// Unix timestamp of unassignment. 0 if still active.
    pub ended_at: i64,
    /// Number of proofs submitted against this assignment.
    pub proof_count: u32,
    /// PDA bump.
    pub bump: u8,
}

#[account]
pub struct Proof {
    /// Assignment this proof is bound to.
    pub assignment: Pubkey,
    /// Sequence number within the assignment (0 = first proof).
    pub sequence: u32,
    /// 32-byte commitment to the proof (e.g. hash of the full proof bytes
    /// stored off-chain, or the proof's public input commitment).
    /// MVP: simple commitment. Production: full Bellman/Groth16 verifier.
    pub commitment: [u8; 32],
    /// Submitter authority.
    pub submitter: Pubkey,
    /// Unix timestamp.
    pub submitted_at: i64,
    /// PDA bump.
    pub bump: u8,
}

impl Proof {
    // assignment(32) + sequence(4) + commitment(32) + submitter(32) + submitted_at(8) + bump(1)
    pub const INIT_SPACE: usize = 32 + 4 + 32 + 32 + 8 + 1;
}

impl Shipment {
    /// Validate that a transition from `from` to `to` is permitted.
    pub fn can_transition(from: ShipmentStatus, to: ShipmentStatus) -> bool {
        use ShipmentStatus::*;
        matches!(
            (from, to),
            (Created, InTransit) | (InTransit, Delivered) | (Delivered, Closed)
        )
    }
}
