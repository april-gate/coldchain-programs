# Changelog

- 2026-05-31 — Add on-chain hash-chain integrity anchor: `Shipment.chain_hash` commits to the full ordered proof history, folded on each `submit_proof`. See [docs/design/PROOF_HASH_CHAIN.md](docs/design/PROOF_HASH_CHAIN.md).
- 2026-05-31 — Add verification attestation layer: `attest_shipment_verification` records additive, multi-party off-chain verification outcomes on closed shipments. See [docs/design/VERIFY_ATTESTATION.md](docs/design/VERIFY_ATTESTATION.md).
