# coldchain-programs

Solana on-chain programs for [April Gate](https://aprilgatehq.com) — tamper-evident cold chain verification for pharmaceutical cargo.

This repository contains the on-chain components: device registry, shipment lifecycle, immutable assignment history, and consensus dispatch records. The off-chain consensus engine lives in [coldchain-core](https://github.com/april-gate/coldchain-core).

> **Colosseum May 2026 submission:** [`colosseum-submission-may2026`](https://github.com/april-gate/coldchain-programs/releases/tag/colosseum-submission-may2026). Devnet program: `APRu6WGxe1NC4X2FrcLpujRRtqLNfMTSt6fYp5wQZVtP`.
>
> Main branch reflects active post-submission development.

## Program

| Name | Program ID |
|---|---|
| `device_registry` | `APRBVwwJJeStD5wShyg4HivneDYj4TCPYKtSFX5F4jez` |

The program is pinned to this address on every cluster. The deploy keypair lives at `APRBVwwJJeStD5wShyg4HivneDYj4TCPYKtSFX5F4jez.json` at the repo root — its filename is its public key, so the keypair's identity is self-evident.

## Architecture

April Gate is structured as three layers, each with a distinct responsibility and trust model:

**On-chain witness (this program).** A Solana program that records the lifecycle of EQS (Ephemeral Quorum Subnet) shipment-monitoring instances: subnet formation, device-to-subnet assignment, consensus dispatches from members, and subnet dissolution. The chain authenticates that submitters are registered, hardware-rooted devices and records their dispatches. It does not arbitrate the subnet's internal consensus protocol.

**Operator authority.** The shipment authority key founds each subnet, maintains the off-chain manifest (route, temperature range, deadlines, lot numbers, quorum policy, expected cluster), and dissolves the subnet when the shipment completes. The chain binds the off-chain manifest via a 32-byte commitment.

**Off-chain data availability.** Operational detail that does not belong on a public chain — full readings, decrypted diagnostics, human-readable context — lives in customer-controlled storage, consumed by insurers, lawyers, and auditors alongside the on-chain dispatch record. This layer lives in [coldchain-core](https://github.com/april-gate/coldchain-core).

The chain alone answers a single factual question: *did registered devices anchor consensus dispatches, in what sequence, between a shipment's start and close transactions?* Whether a given dispatch count meets a shipment's compliance bar is determined off-chain by parties holding the manifest. The chain is evidentiary; interpretation is human.

## Why this design

Cold-chain disputes cost the pharmaceutical industry billions annually. The verification problem is structural: temperature logs are stored centrally, can be edited, backdated, and challenged in court. April Gate replaces thousands of mutable readings with a stream of tamper-evident consensus dispatches anchored on Solana, each signed by a hardware-rooted device identity.

The `device_registry` program records:

1. **Hardware-rooted device identity.** Each device's identity is its ATECC608 secure element serial — burned in at the factory, with a public key whose private half cannot be exfiltrated. Only a device's recorded authority can submit dispatches under its assignment.
2. **Shipment lifecycle.** A subnet is *founded* (`create_shipment`), *active*, then *dissolved* (`close_shipment`, one-way). The chain tracks only this minimal bracketing — not human-interpretive status. Operational lifecycle (in transit, delivered, spoiled) is the operator's off-chain concern.
3. **Immutable assignment history.** Every device-to-shipment assignment creates a new PDA, never overwrites. An insurance investigator querying a shipment 18 months later sees the full chronological history of every device that ever carried it.
4. **Consensus dispatches.** Each round of off-chain quorum consensus produces a 32-byte commitment, anchored on-chain against the shipment. No per-dispatch account is created — the dispatch lives as an event in the transaction log, and the shipment's commitment count is updated in place. On-chain storage per shipment is O(1) regardless of dispatch count.

## Account model

```
DeviceRegistry          PDA: [b"device", device_id]
  ├── authority           32 B
  ├── device_id           32 B  (ATECC608 serial in bytes [0..9], reserved tail)
  ├── pubkey              33 B  (compressed P-256, SEC1)
  ├── assignment_count     4 B
  ├── current_assignment  32 B  (or Pubkey::default if unassigned)
  └── bump                 1 B

Shipment                PDA: [b"shipment", authority, nonce]
  ├── authority           32 B
  ├── nonce               32 B  (unguessable shipment identifier)
  ├── manifest_commitment 32 B  (hash binding the off-chain manifest)
  ├── proof_count          4 B  (consensus dispatches anchored so far)
  ├── last_commitment     32 B  (most recent dispatch's commitment)
  ├── closed               1 B  (bool; one-way terminal flag)
  └── bump                 1 B

DeviceAssignment        PDA: [b"assignment", device, sequence_le_bytes]
  ├── device              32 B
  ├── shipment            32 B
  ├── sequence             4 B  (0 = first assignment for this device)
  ├── authority           32 B
  ├── assigned_at          8 B
  ├── ended_at             8 B  (0 = still active)
  ├── proof_count          4 B  (per-assignment dispatch count, off-chain accounting)
  └── bump                 1 B
```

There is no per-dispatch account. The audit trail of dispatches is the transaction log: each `submit_proof` emits a `ProofSubmitted` event, retrievable via standard Solana RPC (`getSignaturesForAddress` on the shipment PDA, then parse each transaction's events). Solana's `blockTime` provides the authoritative chain-witnessed timestamp.

The PDA seed structures enable native on-chain queries without an off-chain indexer:

- **All devices ever on shipment X** — `getProgramAccounts(DeviceAssignment, memcmp on shipment field)`.
- **Full assignment history for device D** — derive PDAs at sequences `0..device.assignment_count`.

Shipment nonces are 32-byte unguessable values rather than sequential integers, so an observer who knows an operator's authority wallet cannot enumerate the operator's shipments by guessing nonces.

## Instructions

| Instruction | Purpose |
|---|---|
| `register_device` | Create a `DeviceRegistry` PDA from a 32-byte device ID and 33-byte public key |
| `create_shipment` | Found a shipment subnet: a `Shipment` PDA bound to a 32-byte nonce and manifest commitment |
| `assign_device` | Create an immutable `DeviceAssignment` linking device → shipment (rejected if shipment is closed) |
| `end_assignment` | Mark the current assignment as ended (permitted regardless of shipment closed state) |
| `submit_proof` | Anchor a 32-byte consensus dispatch commitment against an active assignment; submitter must be the device's recorded authority |
| `close_shipment` | Dissolve the subnet (authority-only, one-way); after close, no new dispatches or assignments are accepted |

Every state-changing instruction emits an event for off-chain indexing.

## Quick start

### Prerequisites

- Rust + Cargo
- [Solana CLI](https://docs.solanalabs.com/cli/install) (Anza Agave 3.x, platform-tools v1.52+)
- [Anchor](https://www.anchor-lang.com/docs/installation) 0.32+
- Node.js 18+ and Yarn

### Build

Anchor expects the program keypair at `target/deploy/device_registry-keypair.json`. Copy it into place from the canonical location at the repo root:

```bash
yarn install
mkdir -p target/deploy
cp APRBVwwJJeStD5wShyg4HivneDYj4TCPYKtSFX5F4jez.json target/deploy/device_registry-keypair.json
anchor build
```

Verify the program ID matches:

```bash
anchor keys list
# device_registry: APRBVwwJJeStD5wShyg4HivneDYj4TCPYKtSFX5F4jez
```

### Test

The test suite runs against an ephemeral local validator:

```bash
anchor test
```

The suite exercises:

- Device registration with PDA derivation
- Shipment creation with 32-byte nonce and manifest commitment
- Assignment lifecycle: assign → reject duplicate → end → reassign with history preserved
- `getProgramAccounts` + `memcmp` query proving "all devices ever on shipment X" works on-chain
- Consensus dispatch submission: sequence ordering, per-shipment and per-assignment counts, `last_commitment` update
- Hardware-rooted submitter enforcement: dispatches from a non-authority signer are rejected
- Shipment close: one-way terminal flag, rejection of dispatches and assignments after close, `end_assignment` still permitted post-close

### Deploy to devnet

```bash
solana config set --url devnet
solana airdrop 2   # repeat as needed; program deploy costs roughly 5 SOL
anchor deploy --provider.cluster devnet
```

Run the test suite against the live devnet program:

```bash
anchor test --provider.cluster devnet --skip-deploy --skip-local-validator
```

## Project structure

```
.
├── APRBV...json                        Program deploy keypair (= program ID)
├── Anchor.toml                         Anchor workspace config
├── Cargo.toml                          Rust workspace config
├── package.json                        TypeScript test dependencies
├── tsconfig.json
│
├── programs/device-registry/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      #[program] entry point + dispatch
│       ├── state.rs                    Account types
│       ├── errors.rs                   Custom error codes
│       ├── events.rs                   Emitted events
│       └── instructions/
│           ├── mod.rs
│           ├── register_device.rs
│           ├── create_shipment.rs
│           ├── assign_device.rs
│           ├── end_assignment.rs
│           ├── submit_proof.rs
│           └── close_shipment.rs
│
└── tests/
    └── device-registry.ts              Integration tests (mocha + chai)
```

## Roadmap

- **Multi-authority workflows.** Device manufacturer registers and owns devices; logistics operator creates shipments; assignment requires both signatures. Mirrors real-world responsibility split.
- **Cross-vendor device identifiers.** The reserved bytes in `device_id` will encode a vendor namespace tag (ATECC608, NXP A1006, STSAFE, etc.) so the registry can accommodate multiple secure-element families.
- **Open-source verification tooling.** A CLI that reconstructs a shipment's full dispatch history from the transaction log, verifies each dispatch, and prints a compliance verdict against the off-chain manifest — making the on-chain record independently auditable by anyone, without trusting the operator's API.
- **Immutable program authority.** Move the program's upgrade authority to a timelocked multisig before mainnet deploy, so any program change is publicly visible before activation.

## License

Apache 2.0

---

Built for the [Solana Frontier Hackathon](https://colosseum.com/frontier) — Colosseum, May 2026.
