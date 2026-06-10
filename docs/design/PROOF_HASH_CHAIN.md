# Implementation Spec: On-Chain Hash-Chain Integrity Anchor

## Design decision

The `Shipment` PDA will carry a constant-size cryptographic hash chain
that commits to the full history of consensus proofs submitted against
that shipment. Each `submit_proof` instruction updates the chain by
hashing `(prev_chain_hash || new_proof_commitment || proof_sequence)`
and storing the result back into the `Shipment` PDA.

This replaces relying solely on transaction-log event retrieval for
audit reconstruction. Individual proof events still live in the
transaction log (cheap, permanent); the hash chain provides a
constant-size, trivially-queryable on-chain integrity anchor that lets
any party verify the completeness and ordering of retrieved proofs
without trusting the retrieval path.

A per-proof PDA model was prototyped earlier and rejected as
economically unviable at industry scale. This hash-chain approach
preserves the same auditability with constant on-chain storage cost
per shipment.

## What stays the same

- The three PDA types: `DeviceRegistry`, `Shipment`, `DeviceAssignment`.
  Their seeds, derivation, and existing fields are preserved.
- `DeviceAssignment` is unchanged. Every assignment still creates a new
  PDA (immutable history).
- `submit_proof` instruction signature, account constraints, and
  authorization checks (assignment must link submitter device to
  shipment, must be active, etc.) are unchanged.
- The `ProofSubmitted` event continues to be emitted in the transaction
  log — the hash chain *complements* event-log auditability, it does
  not replace it.
- No new PDAs are introduced. (Specifically: do NOT reintroduce a
  per-proof `Proof` PDA — that's the model we deliberately replaced.)

## Changes

### 1. `state/shipment.rs`

Add one field to the `Shipment` struct:

```rust
pub chain_hash: [u8; 32],
```

Place it near `proof_count` and `last_commitment` (whichever currently
exists). Update `Shipment::SPACE` constant to include the new 32 bytes.

At shipment creation (`create_shipment` handler), initialize
`chain_hash` to a domain-separated hash binding the chain to the
shipment's identity:

```rust
use anchor_lang::solana_program::hash::hashv;

let genesis_hash = hashv(&[
    b"april-gate-shipment-genesis-v1",  // domain separator
    shipment.key().as_ref(),             // shipment PDA address
    &shipment.created_at.to_le_bytes(),  // creation timestamp
]);
shipment.chain_hash = genesis_hash.to_bytes();
```

Rationale for binding genesis to shipment identity (rather than just
using zeros): the chain hash is intended to be exportable for
off-chain audit. Two shipments with identical proof histories should
produce different chain hashes from the very first proof onward, so
auditors can never spuriously identify them as equivalent. The PDA
address binding ensures uniqueness; the domain separator prevents
genesis hashes from colliding with any other hashes in the system.

### 2. `instructions/submit_proof.rs`

Inside the handler, after all existing validation passes and after the
`ProofSubmitted` event is emitted, update the chain hash:

```rust
use anchor_lang::solana_program::hash::hashv;

let prev = shipment.chain_hash;
let seq_bytes = shipment.proof_count.to_le_bytes(); // sequence
                                                     // BEFORE
                                                     // increment

let new_hash = hashv(&[
    &prev,
    &proof_commitment,        // the 32-byte commitment being submitted
    &seq_bytes,
]);

shipment.chain_hash = new_hash.to_bytes();
// existing logic: increment proof_count, update last_commitment, etc.
```

Use SHA-256 via `anchor_lang::solana_program::hash::hashv` (it's
Solana's native, well-tested hash and is cheaper in compute units than
pulling in Keccak).

Order of inputs to the hash is critical:
1. `prev_chain_hash` (32 bytes)
2. `proof_commitment` (32 bytes)
3. `proof_sequence` as u32 little-endian bytes (4 bytes)

Including the sequence prevents an attacker from reordering proofs and
producing the same final hash.

### 3. `events.rs`

Add `chain_hash_after` to `ProofSubmitted` so off-chain consumers can
verify chain progression from event data alone:

```rust
#[event]
pub struct ProofSubmitted {
    // ... existing fields ...
    pub chain_hash_after: [u8; 32],
}
```

Populate it in the `submit_proof` handler with the post-update
`chain_hash`.

### 4. Tests

Add tests in the existing test directory:

- **`test_chain_hash_genesis`**: After `create_shipment`, the
  `chain_hash` field equals
  `SHA256("april-gate-shipment-genesis-v1" || shipment_pda || created_at_le_bytes)`.
  Verify by computing the expected genesis hash client-side and
  comparing.
- **`test_chain_hash_single_proof`**: After one `submit_proof`, the
  `chain_hash` equals `SHA256(genesis_hash || commitment || 0_le_bytes)`,
  where `genesis_hash` is computed from the shipment identity per the
  spec above. Verify against a client-side computation.
- **`test_chain_hash_multiple_proofs`**: After N proofs, the
  `chain_hash` matches a client-side replay of the chain. Use N=5.
- **`test_chain_hash_order_dependency`**: Two shipments receiving the
  same commitments in different orders produce *different* final
  `chain_hash` values. (This tests that sequence is folded into the
  hash correctly.)

## Out of scope — do NOT do these

- Do not change `DeviceRegistry` or `DeviceAssignment` structures.
- Do not reintroduce a per-proof PDA.
- Do not change the `submit_proof` authorization logic or account
  constraints.
- Do not change `create_shipment` beyond initializing the new
  `chain_hash` field to zero.
- Do not add a separate "chain verification" instruction — verification
  is meant to happen off-chain by replaying events against the on-chain
  anchor.
- Do not modify the off-chain client (`chain-client/`) in this change.
  That's a separate task once the on-chain change is verified.

## Acceptance criteria

- All existing tests pass unchanged.
- The four new tests above pass.
- `anchor build` succeeds without warnings on the changed files.
- The `Shipment::SPACE` constant correctly reflects the added 32 bytes
  (check that account allocation in `create_shipment` matches).

## Notes for the implementer

- The hashing uses Solana's built-in SHA-256 (`hashv`), not Keccak.
- Genesis state is bound to shipment identity via a domain-separated
  hash (see spec section above) — this is deliberately stronger than
  the conventional all-zeros genesis, because the chain hash is meant
  to be exportable for off-chain audit and should be unique per
  shipment from byte one.
- The sequence used in the per-proof hash step is the `proof_count`
  *before* incrementing (so the first proof uses sequence 0, matching
  the event's sequence number).
