use anchor_lang::prelude::*;

/// 32-byte device identifier.
///
/// Currently: ATECC608 serial number in bytes [0..9], remaining bytes zero.
/// Reserved for future use: type tag, vendor namespace, identifier extensions.
pub type DeviceId = [u8; 32];

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

/// A Shipment is the on-chain bracketing of an EQS (Ephemeral Quorum Subnet)
/// — the permissioned cluster of sensors monitoring one cargo shipment.
///
/// The chain records when the subnet formed (create_shipment), the stream of
/// consensus dispatches it produced (submit_proof), and when it was dissolved
/// (close_shipment). The chain does not arbitrate the subnet's internal
/// consensus protocol — that is the subnet's own business, verifiable by
/// parties holding the operator's off-chain manifest.
#[account]
#[derive(InitSpace)]
pub struct Shipment {
    /// Operator wallet that founded this shipment subnet. Authorized to
    /// assign devices and to close the subnet. May be a per-shipment fresh
    /// keypair (recommended) to break correlation between the operator's
    /// real-world identity and their on-chain shipment history.
    pub authority: Pubkey,
    /// 32-byte unguessable shipment identifier chosen by the operator.
    /// Stored opaquely so an observer who knows `authority` cannot enumerate
    /// the operator's shipments by guessing sequential nonces.
    pub nonce: [u8; 32],
    /// 32-byte hash binding the off-chain shipment manifest (route,
    /// temperature range, deadlines, lot numbers, quorum policy, expected
    /// cluster identifiers). The manifest itself lives off-chain under
    /// operator control; this commitment makes it tamper-evident.
    pub manifest_commitment: [u8; 32],
    /// Total consensus dispatches anchored to this shipment. Chain-assigned
    /// sequence source; incremented on each accepted proof.
    pub proof_count: u32,
    /// 32-byte commitment from the most recent dispatch. Lets an observer
    /// read the latest anchor from a single account fetch.
    pub last_commitment: [u8; 32],
    /// Unix timestamp captured at subnet formation (`create_shipment`). Bound
    /// into the genesis chain hash so the exportable hash chain is unique per
    /// shipment from byte one; also a convenience for off-chain accounting.
    pub created_at: i64,
    /// Constant-size cryptographic hash chain committing to the full ordered
    /// history of consensus proofs anchored to this shipment. Initialized at
    /// creation to a domain-separated genesis hash bound to the shipment's
    /// identity, then folded forward on every accepted proof
    /// (`SHA256(prev_chain_hash || proof_commitment || proof_sequence)`).
    /// Provides a constant-size, trivially-queryable on-chain integrity anchor
    /// that lets any party verify the completeness and ordering of proofs
    /// retrieved from the transaction log without trusting the retrieval path.
    pub chain_hash: [u8; 32],
    /// True once the operator has dissolved the subnet. One-way: cannot be
    /// unset. When true, the program rejects new proofs and new assignments.
    pub closed: bool,
    /// PDA bump for re-derivation.
    pub bump: u8,
    /// Independent off-chain verification attestations pinned to this
    /// shipment's `chain_hash`. Additive and multi-party: any signer may
    /// attest a closed shipment; downstream consumers choose which verifiers
    /// to trust. Stored inline (rather than per-attestation PDAs) so the full
    /// verification status is readable in a single `getAccountInfo` call.
    // max_len must match Shipment::MAX_ATTESTATIONS (kept as a literal here
    // because the InitSpace derive macro expects a literal length).
    #[max_len(8)]
    pub attestations: Vec<VerificationAttestation>,
}

impl Shipment {
    /// Domain separator for the genesis chain hash. Versioned so a future
    /// change to the genesis construction cannot collide with existing chains.
    pub const GENESIS_DOMAIN: &'static [u8] = b"april-gate-shipment-genesis-v1";
    /// Reserved attestation capacity per shipment. Covers the realistic
    /// multi-party ceiling (insurer + manufacturer + regulator + April Gate +
    /// buffer) without unbounded account growth.
    pub const MAX_ATTESTATIONS: usize = 8;
}

/// Outcome a verifier records when attesting a shipment's proof history.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum VerificationOutcome {
    /// History verified; no violations found.
    Passed,
    /// A temperature/condition excursion was detected.
    FailedExcursion,
    /// Quorum requirements were not met across the proof history.
    FailedQuorum,
    /// The hash chain / proof set failed integrity reconstruction.
    FailedIntegrity,
    /// Verification could not reach a conclusion (e.g. missing data).
    Inconclusive,
}

/// A single off-chain verification attestation, stored inline on `Shipment`.
///
/// `attestation_signature` is the verifier's signature over the attestation
/// payload (shipment_pubkey || chain_hash_at_verification || outcome_byte ||
/// verified_at). It is recorded on-chain so downstream consumers can verify
/// provenance off-chain; see `attest_shipment_verification` for the (currently
/// deferred) on-chain ed25519 verification path.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, InitSpace)]
pub struct VerificationAttestation {
    /// Verifier identity (the attesting signer).
    pub verifier: Pubkey,
    /// Unix timestamp the attestation was recorded on-chain.
    pub verified_at: i64,
    /// The `chain_hash` value the verifier observed when verifying. Pins the
    /// attestation to a specific point in the shipment's proof history.
    pub chain_hash_at_verification: [u8; 32],
    /// The recorded verification outcome.
    pub outcome: VerificationOutcome,
    /// Verifier signature over the attestation payload.
    pub attestation_signature: [u8; 64],
}

impl VerificationAttestation {
    // verifier(32) + verified_at(8) + chain_hash_at_verification(32)
    // + outcome(1) + attestation_signature(64)
    pub const SPACE: usize = 32 + 8 + 32 + 1 + 64; // 137 bytes
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
    /// Number of proofs submitted against this assignment. Maintained for
    /// off-chain per-assignment accounting; not used as a PDA seed.
    pub proof_count: u32,
    /// PDA bump.
    pub bump: u8,
}
