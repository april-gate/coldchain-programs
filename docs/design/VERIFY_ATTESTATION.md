# Implementation Spec: Verification Attestation Layer

## Context: read this *together with* `HASH_CHAIN_IMPLEMENTATION.md`

This change builds on the hash-chain implementation. The two specs are
intended to be implemented in the same change, on the same branch, in
one sweep. The hash chain provides the cryptographic anchor that
attestations pin to; the attestation layer provides the on-chain
artifact that records off-chain verification.

If implementing both at once: do the hash chain first (it's the
foundation), then the attestation layer (which references
`chain_hash`).

## Design decision

After a shipment is closed, one or more verifiers can submit signed
**attestations** that they have independently verified the shipment's
proof history off-chain and recorded the outcome. The attestation is
stored on the `Shipment` PDA so any party can read the verification
status trivially via `getAccountInfo` — no archival lookup needed for
operational use (parametric insurance payout, customer settlement,
compliance sign-off).

Attestations are **additive and multi-party**. The architecture does
not designate a single privileged verifier. Different verifiers
(insurer's compliance system, manufacturer's QA, neutral third-party,
April Gate's own verifier service) can each submit their own
attestation. Downstream consumers choose which attestor(s) they trust.

Attestations are **not required** to close a shipment. Shipment close
remains a separate, independent action. Attestation happens
afterward, ideally within a short window when transaction retrieval
is cheap — but the on-chain primitive does not enforce timing.

## What stays the same

- All hash-chain logic from the companion spec.
- `Shipment` close behavior — closing a shipment does not require an
  attestation.
- `DeviceRegistry`, `DeviceAssignment` — unchanged.
- `submit_proof` — unchanged. Attestations do not affect proof
  submission.

## Changes

### 1. New types in `state/shipment.rs`

Add the attestation types alongside `Shipment`:

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerificationOutcome {
    Passed,
    FailedExcursion,
    FailedQuorum,
    FailedIntegrity,
    Inconclusive,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct VerificationAttestation {
    pub verifier:                   Pubkey,    // who attested
    pub verified_at:                i64,       // unix timestamp
    pub chain_hash_at_verification: [u8; 32],  // chain_hash the
                                                // verifier saw
    pub outcome:                    VerificationOutcome,
    pub attestation_signature:      [u8; 64],  // verifier signature
                                                // over the
                                                // attestation payload
}

impl VerificationAttestation {
    pub const SPACE: usize = 32 + 8 + 32 + 1 + 64; // 137 bytes
}
```

### 2. Add `attestations` field to `Shipment`

```rust
pub attestations: Vec<VerificationAttestation>,
```

Place it after the `chain_hash` field added in the hash-chain spec.

**Size budget:** Reserve space for up to **8 attestations per
shipment**. This covers the realistic ceiling: insurer + manufacturer
+ regulator + April Gate + a few buffer slots for unforeseen
multi-party flows. 8 × 137 bytes = 1,096 bytes. Plus 4 bytes for the
Vec length prefix.

Update `Shipment::SPACE` accordingly:

```rust
impl Shipment {
    pub const MAX_ATTESTATIONS: usize = 8;
    pub const SPACE: usize =
        /* existing fields */ +
        32 /* chain_hash from hash-chain spec */ +
        4 /* Vec length prefix */ +
        Self::MAX_ATTESTATIONS * VerificationAttestation::SPACE;
}
```

Initialize `attestations` to `Vec::new()` in `create_shipment`.

### 3. New instruction: `attest_shipment_verification`

Create `instructions/attest_shipment_verification.rs`:

```rust
use anchor_lang::prelude::*;
use anchor_lang::solana_program::ed25519_program;
use anchor_lang::solana_program::sysvar::instructions::{
    self, load_current_index_checked, load_instruction_at_checked,
};

use crate::errors::DeviceRegistryError;
use crate::events::ShipmentAttested;
use crate::state::{Shipment, VerificationAttestation, VerificationOutcome};

#[derive(Accounts)]
pub struct AttestShipmentVerification<'info> {
    #[account(
        mut,
        seeds = [b"shipment", shipment.authority.as_ref(), &shipment.nonce.to_le_bytes()],
        bump  = shipment.bump,
    )]
    pub shipment: Account<'info, Shipment>,

    /// The verifier — anyone can attest. Authorization at the
    /// attestation-trust level happens off-chain (downstream consumers
    /// choose which verifier pubkeys they trust).
    #[account(mut)]
    pub verifier: Signer<'info>,

    /// CHECK: Sysvar for cross-instruction signature verification of
    /// the attestation payload.
    #[account(address = instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<AttestShipmentVerification>,
    chain_hash_at_verification: [u8; 32],
    outcome: VerificationOutcome,
    attestation_signature: [u8; 64],
) -> Result<()> {
    let shipment = &mut ctx.accounts.shipment;

    // 1. Shipment must be closed before it can be attested.
    require!(
        shipment.closed,
        DeviceRegistryError::ShipmentNotClosed
    );

    // 2. Capacity check.
    require!(
        shipment.attestations.len() < Shipment::MAX_ATTESTATIONS,
        DeviceRegistryError::AttestationCapacityExceeded
    );

    // 3. Verify the attestation signature via the ed25519 sysvar.
    //    The signed payload is:
    //      shipment_pubkey || chain_hash_at_verification ||
    //      outcome_byte || verified_at_le_bytes
    //    Implementer: see Solana docs for ed25519 sig verification via
    //    the instructions sysvar. The verifier signs off-chain, then
    //    includes both the ed25519 verify instruction *and* this
    //    instruction in the same transaction. Validate that the
    //    sysvar contains a matching ed25519 verify instruction whose
    //    pubkey == verifier.key() and whose signed message matches the
    //    expected payload above.

    let clock = Clock::get()?;
    let verified_at = clock.unix_timestamp;

    let attestation = VerificationAttestation {
        verifier: ctx.accounts.verifier.key(),
        verified_at,
        chain_hash_at_verification,
        outcome,
        attestation_signature,
    };

    shipment.attestations.push(attestation);

    emit!(ShipmentAttested {
        shipment: shipment.key(),
        verifier: ctx.accounts.verifier.key(),
        verified_at,
        chain_hash_at_verification,
        outcome,
    });

    Ok(())
}
```

**On the signature verification:** Solana programs cannot directly
verify ed25519 signatures in BPF — the standard pattern is to require
the caller to include an ed25519 verify instruction in the same
transaction, then read the instructions sysvar to confirm the
verification was done with the expected pubkey and message. If this
adds complexity beyond what the spec naturally accommodates,
implement the basic version (store the signature, defer verification
to off-chain consumers) and flag it with a `TODO` for follow-up. The
storage and structure are the load-bearing part of this change; the
verification mechanism can be hardened separately.

### 4. New error variants in `errors.rs`

```rust
#[msg("Shipment must be closed before it can be attested")]
ShipmentNotClosed,

#[msg("Shipment has reached its maximum attestation capacity")]
AttestationCapacityExceeded,
```

### 5. New event in `events.rs`

```rust
#[event]
pub struct ShipmentAttested {
    pub shipment:                   Pubkey,
    pub verifier:                   Pubkey,
    pub verified_at:                i64,
    pub chain_hash_at_verification: [u8; 32],
    pub outcome:                    VerificationOutcome,
}
```

### 6. Tests

Add tests in the existing test directory:

- **`test_attest_closed_shipment`**: Close a shipment, then attest.
  Verify the attestation appears in `shipment.attestations` with
  correct fields.
- **`test_attest_open_shipment_fails`**: Try to attest a shipment
  that hasn't been closed. Should fail with `ShipmentNotClosed`.
- **`test_multiple_attestations_different_verifiers`**: Two distinct
  verifier keypairs each attest the same closed shipment. Both
  attestations should be present in the vector.
- **`test_attestation_capacity_limit`**: Submit 8 attestations
  successfully, then try a 9th. Should fail with
  `AttestationCapacityExceeded`.
- **`test_attestation_records_chain_hash`**: Attest a shipment, verify
  that `chain_hash_at_verification` in the stored attestation matches
  the actual `chain_hash` on the Shipment at attestation time.

## Out of scope — do NOT do these

- Do not enforce that attestations must happen within a time window
  after shipment close. Timing is an off-chain practice, not an
  on-chain rule.
- Do not enforce a specific verifier identity or whitelist. Any signer
  can attest. Trust in specific verifiers is an off-chain concern.
- Do not implement the off-chain verifier itself. That belongs in
  `coldchain-verifier` (separate repo, future work).
- Do not change `close_shipment` to require attestation.
- Do not deduplicate attestations from the same verifier. A verifier
  might legitimately re-attest after additional analysis. Downstream
  consumers can choose which attestation to use.

## Acceptance criteria

- All existing tests pass unchanged.
- All hash-chain tests (from the companion spec) pass.
- The five new attestation tests above pass.
- `anchor build` succeeds without warnings.
- `Shipment::SPACE` correctly accounts for the new fields including
  the reserved attestation capacity.

## Notes for the implementer

- This spec is to be implemented *together* with
  `HASH_CHAIN_IMPLEMENTATION.md` in one branch. Do the hash chain
  first; this layer references `chain_hash` in attestations.
- The ed25519 signature verification via sysvar is the most complex
  piece. If it materially expands scope, implement the storage and
  structure faithfully and leave the cross-instruction signature
  verification as a TODO — it can be hardened in a follow-up without
  changing account layout.
- Attestations are deliberately stored in a Vec on the Shipment PDA
  rather than in separate per-attestation PDAs, because (a) the
  expected count per shipment is small (1–3 typically, 8 ceiling),
  and (b) keeping attestations local to the shipment makes them
  trivially queryable with a single `getAccountInfo` call — which is
  the operational property the design is trying to achieve.
