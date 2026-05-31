import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DeviceRegistry } from "../target/types/device_registry";

import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
} from "@solana/web3.js";
import { assert } from "chai";
import * as crypto from "crypto";

describe("device-registry", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.deviceRegistry as Program<DeviceRegistry>;
  const authority = provider.wallet as anchor.Wallet;

  // Helpers ──────────────────────────────────────────────────────────────────

  /** Generate a 32-byte device_id whose first 9 bytes look like an ATECC serial. */
  function makeDeviceId(): number[] {
    const id = Buffer.alloc(32);
    id[0] = 0x01;
    id[1] = 0x23;
    crypto.randomBytes(6).copy(id, 2);
    id[8] = 0xee;
    return Array.from(id);
  }

  /** Mock compressed P-256 public key (33 bytes). For tests; not validated on-chain. */
  function makePubkey(): number[] {
    const pk = Buffer.alloc(33);
    pk[0] = 0x02;
    crypto.randomBytes(32).copy(pk, 1);
    return Array.from(pk);
  }

  /** Random 32-byte value (used for both nonces and commitments). */
  function rand32(): number[] {
    return Array.from(crypto.randomBytes(32));
  }

    /**
     * Fund a keypair by transferring from the provider wallet, avoiding the
     * devnet airdrop faucet (which is rate-limited and fails in CI / repeated runs).
     * Works identically on localnet and devnet.
     */
    async function fundFromProvider(target: PublicKey, lamports: number) {
        const tx = new anchor.web3.Transaction().add(
            SystemProgram.transfer({
                fromPubkey: authority.publicKey,
                toPubkey: target,
                lamports,
            })
        );
        await provider.sendAndConfirm(tx);
    }


  function devicePda(deviceId: number[]): PublicKey {
    const [pda] = PublicKey.findProgramAddressSync(
      [Buffer.from("device"), Buffer.from(deviceId)],
      program.programId
    );
    return pda;
  }

  function shipmentPda(authority: PublicKey, nonce: number[]): PublicKey {
    const [pda] = PublicKey.findProgramAddressSync(
      [Buffer.from("shipment"), authority.toBuffer(), Buffer.from(nonce)],
      program.programId
    );
    return pda;
  }

  function assignmentPda(device: PublicKey, sequence: number): PublicKey {
    const seqBuf = Buffer.alloc(4);
    seqBuf.writeUInt32LE(sequence, 0);
    const [pda] = PublicKey.findProgramAddressSync(
      [Buffer.from("assignment"), device.toBuffer(), seqBuf],
      program.programId
    );
    return pda;
  }

  /**
   * Register a device, create a shipment, and assign the device to it.
   * Returns the keys needed to drive submit_proof / close_shipment tests.
   */
  async function setupAssignedDevice() {
    const deviceId = makeDeviceId();
    const pubkey = makePubkey();
    const device = devicePda(deviceId);
    const nonce = rand32();
    const shipment = shipmentPda(authority.publicKey, nonce);
    const manifest = rand32();

    await program.methods
      .registerDevice(deviceId, pubkey)
      .accounts({
        device,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await program.methods
      .createShipment(nonce, manifest)
      .accounts({
        shipment,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const assignment = assignmentPda(device, 0);
    await program.methods
      .assignDevice()
      .accounts({
        device,
        shipment,
        assignment,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    return { deviceId, device, nonce, shipment, assignment, manifest };
  }

  /** SHA-256 of the concatenation of the given buffers (mirrors on-chain hashv). */
  function sha256(...parts: Buffer[]): Buffer {
    const h = crypto.createHash("sha256");
    for (const p of parts) h.update(p);
    return h.digest();
  }

  /**
   * Client-side replica of the on-chain genesis chain hash:
   *   SHA256("april-gate-shipment-genesis-v1" || shipment_pda || created_at_le)
   */
  function genesisChainHash(shipmentPda: PublicKey, createdAt: anchor.BN): Buffer {
    return sha256(
      Buffer.from("april-gate-shipment-genesis-v1"),
      shipmentPda.toBuffer(),
      createdAt.toArrayLike(Buffer, "le", 8)
    );
  }

  /**
   * Client-side replica of one on-chain chain fold:
   *   SHA256(prev_chain_hash || proof_commitment || sequence_u32_le)
   */
  function foldProof(prev: Buffer, commitment: number[], sequence: number): Buffer {
    const seq = Buffer.alloc(4);
    seq.writeUInt32LE(sequence, 0);
    return sha256(prev, Buffer.from(commitment), seq);
  }

  /** Submit a single proof commitment against an assigned device's shipment. */
  async function submitProof(ctx: any, commitment: number[]) {
    await program.methods
      .submitProof(commitment)
      .accounts({
        assignment: ctx.assignment,
        shipment: ctx.shipment,
        device: ctx.device,
        submitter: authority.publicKey,
      })
      .rpc();
  }

  /** Set up an assigned device, submit `nProofs` proofs, then close the shipment. */
  async function setupClosedShipment(nProofs = 2) {
    const ctx = await setupAssignedDevice();
    for (let i = 0; i < nProofs; i++) {
      await submitProof(ctx, rand32());
    }
    await program.methods
      .closeShipment()
      .accounts({ shipment: ctx.shipment, authority: authority.publicKey })
      .rpc();
    return ctx;
  }

  /** Attest a shipment as `verifier`. Defaults to the provider wallet. */
  async function attest(
    shipment: PublicKey,
    chainHashAtVerification: number[],
    outcome: any,
    verifier?: Keypair
  ) {
    const verifierKey = verifier ? verifier.publicKey : authority.publicKey;
    const signature = Array.from(crypto.randomBytes(64));
    const builder = program.methods
      .attestShipmentVerification(chainHashAtVerification, outcome, signature)
      .accounts({
        shipment,
        verifier: verifierKey,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      });
    if (verifier) builder.signers([verifier]);
    await builder.rpc();
  }

  // Device ─────────────────────────────────────────────────────────────────────

  describe("register_device", () => {
    it("registers a device", async () => {
      const deviceId = makeDeviceId();
      const pubkey = makePubkey();
      const device = devicePda(deviceId);

      await program.methods
        .registerDevice(deviceId, pubkey)
        .accounts({
          device,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const acct = await program.account.deviceRegistry.fetch(device);
      assert.deepEqual(acct.deviceId, deviceId);
      assert.deepEqual(acct.pubkey, pubkey);
      assert.equal(acct.assignmentCount, 0);
      assert.ok(acct.authority.equals(authority.publicKey));
      assert.ok(acct.currentAssignment.equals(PublicKey.default));
    });
  });

  // Shipment ───────────────────────────────────────────────────────────────────

  describe("create_shipment", () => {
    it("creates a shipment with 32-byte nonce and manifest commitment", async () => {
      const nonce = rand32();
      const manifest = rand32();
      const shipment = shipmentPda(authority.publicKey, nonce);

      await program.methods
        .createShipment(nonce, manifest)
        .accounts({
          shipment,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const acct = await program.account.shipment.fetch(shipment);
      assert.deepEqual(acct.nonce, nonce);
      assert.deepEqual(acct.manifestCommitment, manifest);
      assert.equal(acct.proofCount, 0);
      assert.equal(acct.closed, false);
      assert.ok(acct.authority.equals(authority.publicKey));
    });

    it("derives distinct PDAs for same nonce under different authorities", async () => {
      // Privacy property: knowing the authority + guessing nonces is not
      // enough to enumerate, because the 32-byte nonce is unguessable.
      // This just demonstrates seed independence.
      const nonce = rand32();
      const other = Keypair.generate();
      const pda1 = shipmentPda(authority.publicKey, nonce);
      const pda2 = shipmentPda(other.publicKey, nonce);
      assert.notEqual(pda1.toBase58(), pda2.toBase58());
    });
  });

  // Assignment ─────────────────────────────────────────────────────────────────

  describe("assign_device / end_assignment", () => {
    it("runs the assignment lifecycle: assign → reject duplicate → end → reassign with history", async () => {
      const deviceId = makeDeviceId();
      const pubkey = makePubkey();
      const device = devicePda(deviceId);
      await program.methods
        .registerDevice(deviceId, pubkey)
        .accounts({
          device,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const nonceA = rand32();
      const shipmentA = shipmentPda(authority.publicKey, nonceA);
      await program.methods
        .createShipment(nonceA, rand32())
        .accounts({
          shipment: shipmentA,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const assignment0 = assignmentPda(device, 0);
      await program.methods
        .assignDevice()
        .accounts({
          device,
          shipment: shipmentA,
          assignment: assignment0,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      let deviceAcct = await program.account.deviceRegistry.fetch(device);
      assert.equal(deviceAcct.assignmentCount, 1);
      assert.ok(deviceAcct.currentAssignment.equals(assignment0));

      // Cannot reassign while active.
      const nonceB = rand32();
      const shipmentB = shipmentPda(authority.publicKey, nonceB);
      await program.methods
        .createShipment(nonceB, rand32())
        .accounts({
          shipment: shipmentB,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const assignment1 = assignmentPda(device, 1);
      try {
        await program.methods
          .assignDevice()
          .accounts({
            device,
            shipment: shipmentB,
            assignment: assignment1,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        assert.fail("expected DeviceAlreadyAssigned");
      } catch (err: any) {
        assert.match(err.toString(), /DeviceAlreadyAssigned/);
      }

      // End the first assignment.
      await program.methods
        .endAssignment()
        .accounts({
          device,
          assignment: assignment0,
          authority: authority.publicKey,
        })
        .rpc();

      deviceAcct = await program.account.deviceRegistry.fetch(device);
      assert.ok(deviceAcct.currentAssignment.equals(PublicKey.default));
      const ended = await program.account.deviceAssignment.fetch(assignment0);
      assert.notEqual(ended.endedAt.toNumber(), 0);

      // Reassign to shipment B (sequence 1).
      await program.methods
        .assignDevice()
        .accounts({
          device,
          shipment: shipmentB,
          assignment: assignment1,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      deviceAcct = await program.account.deviceRegistry.fetch(device);
      assert.equal(deviceAcct.assignmentCount, 2);
      assert.ok(deviceAcct.currentAssignment.equals(assignment1));

      const hist0 = await program.account.deviceAssignment.fetch(assignment0);
      const hist1 = await program.account.deviceAssignment.fetch(assignment1);
      assert.notEqual(hist0.endedAt.toNumber(), 0);
      assert.equal(hist1.endedAt.toNumber(), 0);
      assert.ok(hist0.shipment.equals(shipmentA));
      assert.ok(hist1.shipment.equals(shipmentB));
    });

    it("getProgramAccounts finds all assignments for a shipment", async () => {
      // "Show me every device that was ever on this shipment."
      const nonce = rand32();
      const shipment = shipmentPda(authority.publicKey, nonce);
      await program.methods
        .createShipment(nonce, rand32())
        .accounts({
          shipment,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      for (let i = 0; i < 2; i++) {
        const deviceId = makeDeviceId();
        const device = devicePda(deviceId);
        await program.methods
          .registerDevice(deviceId, makePubkey())
          .accounts({
            device,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();

        const assignment = assignmentPda(device, 0);
        await program.methods
          .assignDevice()
          .accounts({
            device,
            shipment,
            assignment,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
      }

      // Offset = 8 (discriminator) + 32 (device) → shipment field.
      const assignments = await program.account.deviceAssignment.all([
        { memcmp: { offset: 8 + 32, bytes: shipment.toBase58() } },
      ]);

      assert.equal(assignments.length, 2);
      for (const a of assignments) {
        assert.ok(a.account.shipment.equals(shipment));
      }
    });
  });

  // Proof ──────────────────────────────────────────────────────────────────────

  describe("submit_proof", () => {
    it("accepts a dispatch, increments counts, sets last_commitment, no Proof PDA", async () => {
      const ctx = await setupAssignedDevice();
      const commitment = rand32();

      await program.methods
        .submitProof(commitment)
        .accounts({
          assignment: ctx.assignment,
          shipment: ctx.shipment,
          device: ctx.device,
          submitter: authority.publicKey,
        })
        .rpc();

      const shipmentAcct = await program.account.shipment.fetch(ctx.shipment);
      const assignAcct = await program.account.deviceAssignment.fetch(ctx.assignment);
      assert.equal(shipmentAcct.proofCount, 1);
      assert.equal(assignAcct.proofCount, 1);
      assert.deepEqual(shipmentAcct.lastCommitment, commitment);
    });

    it("assigns sequence in submission order across multiple dispatches", async () => {
      const ctx = await setupAssignedDevice();
      const N = 4;
      let last: number[] = [];
      for (let i = 0; i < N; i++) {
        last = rand32();
        await program.methods
          .submitProof(last)
          .accounts({
            assignment: ctx.assignment,
            shipment: ctx.shipment,
            device: ctx.device,
            submitter: authority.publicKey,
          })
          .rpc();
      }
      const shipmentAcct = await program.account.shipment.fetch(ctx.shipment);
      assert.equal(shipmentAcct.proofCount, N);
      assert.deepEqual(shipmentAcct.lastCommitment, last);
    });

    it("rejects a dispatch from an unauthorized submitter", async () => {
      const ctx = await setupAssignedDevice();
        const wrong = Keypair.generate();
        await fundFromProvider(wrong.publicKey, 1e7); // 0.01 SOL, plenty for one failed tx

      try {
        await program.methods
          .submitProof(rand32())
          .accounts({
            assignment: ctx.assignment,
            shipment: ctx.shipment,
            device: ctx.device,
            submitter: wrong.publicKey,
          })
          .signers([wrong])
          .rpc();
        assert.fail("expected UnauthorizedSubmitter");
      } catch (err: any) {
        assert.match(err.toString(), /UnauthorizedSubmitter/);
      }
    });
  });

  // Close ──────────────────────────────────────────────────────────────────────

  describe("close_shipment", () => {
    it("closes a shipment and freezes proof_count in the event", async () => {
      const ctx = await setupAssignedDevice();
      for (let i = 0; i < 3; i++) {
        await program.methods
          .submitProof(rand32())
          .accounts({
            assignment: ctx.assignment,
            shipment: ctx.shipment,
            device: ctx.device,
            submitter: authority.publicKey,
          })
          .rpc();
      }

      await program.methods
        .closeShipment()
        .accounts({ shipment: ctx.shipment, authority: authority.publicKey })
        .rpc();

      const acct = await program.account.shipment.fetch(ctx.shipment);
      assert.equal(acct.closed, true);
      assert.equal(acct.proofCount, 3);
    });

    it("is one-way: a second close fails", async () => {
      const ctx = await setupAssignedDevice();
      await program.methods
        .closeShipment()
        .accounts({ shipment: ctx.shipment, authority: authority.publicKey })
        .rpc();
      try {
        await program.methods
          .closeShipment()
          .accounts({ shipment: ctx.shipment, authority: authority.publicKey })
          .rpc();
        assert.fail("expected ShipmentClosed");
      } catch (err: any) {
        assert.match(err.toString(), /ShipmentClosed/);
      }
    });

    it("rejects close from a non-authority signer", async () => {
      const ctx = await setupAssignedDevice();
        const wrong = Keypair.generate();
        await fundFromProvider(wrong.publicKey, 1e7); // 0.01 SOL, plenty for one failed tx

      try {
        await program.methods
          .closeShipment()
          .accounts({ shipment: ctx.shipment, authority: wrong.publicKey })
          .signers([wrong])
          .rpc();
        assert.fail("expected has_one constraint violation");
      } catch (err: any) {
        // Anchor's has_one violation surfaces as a constraint error.
        assert.match(err.toString(), /ConstraintHasOne|has_one|2001/i);
      }
    });

    it("rejects submit_proof after close", async () => {
      const ctx = await setupAssignedDevice();
      await program.methods
        .closeShipment()
        .accounts({ shipment: ctx.shipment, authority: authority.publicKey })
        .rpc();
      try {
        await program.methods
          .submitProof(rand32())
          .accounts({
            assignment: ctx.assignment,
            shipment: ctx.shipment,
            device: ctx.device,
            submitter: authority.publicKey,
          })
          .rpc();
        assert.fail("expected ShipmentClosed");
      } catch (err: any) {
        assert.match(err.toString(), /ShipmentClosed/);
      }
    });

    it("allows end_assignment after close (devices can be freed)", async () => {
      const ctx = await setupAssignedDevice();
      await program.methods
        .closeShipment()
        .accounts({ shipment: ctx.shipment, authority: authority.publicKey })
        .rpc();
      // Should succeed — end_assignment does not depend on shipment.closed.
      await program.methods
        .endAssignment()
        .accounts({
          device: ctx.device,
          assignment: ctx.assignment,
          authority: authority.publicKey,
        })
        .rpc();
      const deviceAcct = await program.account.deviceRegistry.fetch(ctx.device);
      assert.ok(deviceAcct.currentAssignment.equals(PublicKey.default));
    });

    it("rejects assign_device after close", async () => {
      const ctx = await setupAssignedDevice();
      await program.methods
        .closeShipment()
        .accounts({ shipment: ctx.shipment, authority: authority.publicKey })
        .rpc();

      const newDeviceId = makeDeviceId();
      const newDevice = devicePda(newDeviceId);
      await program.methods
        .registerDevice(newDeviceId, makePubkey())
        .accounts({
          device: newDevice,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const newAssignment = assignmentPda(newDevice, 0);
      try {
        await program.methods
          .assignDevice()
          .accounts({
            device: newDevice,
            shipment: ctx.shipment,
            assignment: newAssignment,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        assert.fail("expected ShipmentClosed");
      } catch (err: any) {
        assert.match(err.toString(), /ShipmentClosed/);
      }
    });
  });

  // Hash chain ───────────────────────────────────────────────────────────────────

  describe("chain_hash", () => {
    it("test_chain_hash_genesis: genesis equals SHA256(domain || pda || created_at)", async () => {
      const ctx = await setupAssignedDevice();
      const acct = await program.account.shipment.fetch(ctx.shipment);
      const expected = genesisChainHash(ctx.shipment, acct.createdAt);
      assert.deepEqual(Buffer.from(acct.chainHash), expected);
    });

    it("test_chain_hash_single_proof: chain folds one commitment at sequence 0", async () => {
      const ctx = await setupAssignedDevice();
      const before = await program.account.shipment.fetch(ctx.shipment);
      const genesis = genesisChainHash(ctx.shipment, before.createdAt);

      const commitment = rand32();
      await submitProof(ctx, commitment);

      const after = await program.account.shipment.fetch(ctx.shipment);
      const expected = foldProof(genesis, commitment, 0);
      assert.deepEqual(Buffer.from(after.chainHash), expected);
    });

    it("test_chain_hash_multiple_proofs: chain matches a client-side replay of N=5", async () => {
      const ctx = await setupAssignedDevice();
      const acct0 = await program.account.shipment.fetch(ctx.shipment);

      let expected = genesisChainHash(ctx.shipment, acct0.createdAt);
      const N = 5;
      for (let seq = 0; seq < N; seq++) {
        const commitment = rand32();
        await submitProof(ctx, commitment);
        expected = foldProof(expected, commitment, seq);
      }

      const acct = await program.account.shipment.fetch(ctx.shipment);
      assert.equal(acct.proofCount, N);
      assert.deepEqual(Buffer.from(acct.chainHash), expected);
    });

    it("test_chain_hash_order_dependency: same commitments in different order → different final hash", async () => {
      const a = await setupAssignedDevice();
      const b = await setupAssignedDevice();

      const c1 = rand32();
      const c2 = rand32();

      await submitProof(a, c1);
      await submitProof(a, c2);

      await submitProof(b, c2);
      await submitProof(b, c1);

      const acctA = await program.account.shipment.fetch(a.shipment);
      const acctB = await program.account.shipment.fetch(b.shipment);

      assert.notDeepEqual(Buffer.from(acctA.chainHash), Buffer.from(acctB.chainHash));

      // And each independently matches its own ordered client-side replay,
      // confirming the difference is driven by the folded sequence, not noise.
      const genA = genesisChainHash(a.shipment, acctA.createdAt);
      const expectedA = foldProof(foldProof(genA, c1, 0), c2, 1);
      assert.deepEqual(Buffer.from(acctA.chainHash), expectedA);

      const genB = genesisChainHash(b.shipment, acctB.createdAt);
      const expectedB = foldProof(foldProof(genB, c2, 0), c1, 1);
      assert.deepEqual(Buffer.from(acctB.chainHash), expectedB);
    });

    it("emits chain_hash_after on ProofSubmitted matching the stored chain hash", async () => {
      const ctx = await setupAssignedDevice();

      const listener = new Promise<any>((resolve) => {
        const id = program.addEventListener("proofSubmitted", (ev) => {
          program.removeEventListener(id);
          resolve(ev);
        });
      });

      await submitProof(ctx, rand32());
      const ev = await listener;

      const acct = await program.account.shipment.fetch(ctx.shipment);
      assert.deepEqual(Buffer.from(ev.chainHashAfter), Buffer.from(acct.chainHash));
    });
  });

  // Attestation ──────────────────────────────────────────────────────────────────

  describe("attest_shipment_verification", () => {
    it("test_attest_closed_shipment: records the attestation with correct fields", async () => {
      const ctx = await setupClosedShipment();
      const acctBefore = await program.account.shipment.fetch(ctx.shipment);
      const chainHash = acctBefore.chainHash;

      await attest(ctx.shipment, chainHash, { passed: {} });

      const acct = await program.account.shipment.fetch(ctx.shipment);
      assert.equal(acct.attestations.length, 1);
      const att = acct.attestations[0];
      assert.ok(att.verifier.equals(authority.publicKey));
      assert.deepEqual(att.chainHashAtVerification, chainHash);
      assert.deepEqual(att.outcome, { passed: {} });
      assert.ok(att.verifiedAt.toNumber() > 0);
    });

    it("test_attest_open_shipment_fails: rejects with ShipmentNotClosed", async () => {
      const ctx = await setupAssignedDevice(); // not closed
      const acct = await program.account.shipment.fetch(ctx.shipment);
      try {
        await attest(ctx.shipment, acct.chainHash, { passed: {} });
        assert.fail("expected ShipmentNotClosed");
      } catch (err: any) {
        assert.match(err.toString(), /ShipmentNotClosed/);
      }
    });

    it("test_multiple_attestations_different_verifiers: both attestations are present", async () => {
      const ctx = await setupClosedShipment();
      const acct = await program.account.shipment.fetch(ctx.shipment);
      const chainHash = acct.chainHash;

      const v1 = Keypair.generate();
      const v2 = Keypair.generate();
      await fundFromProvider(v1.publicKey, 1e7);
      await fundFromProvider(v2.publicKey, 1e7);

      await attest(ctx.shipment, chainHash, { passed: {} }, v1);
      await attest(ctx.shipment, chainHash, { failedExcursion: {} }, v2);

      const after = await program.account.shipment.fetch(ctx.shipment);
      assert.equal(after.attestations.length, 2);
      const verifiers = after.attestations.map((a: any) => a.verifier.toBase58());
      assert.include(verifiers, v1.publicKey.toBase58());
      assert.include(verifiers, v2.publicKey.toBase58());
    });

    it("test_attestation_capacity_limit: 8 succeed, the 9th fails", async () => {
      const ctx = await setupClosedShipment();
      const acct = await program.account.shipment.fetch(ctx.shipment);
      const chainHash = acct.chainHash;

      // No dedup by verifier — the same signer may attest repeatedly.
      for (let i = 0; i < 8; i++) {
        await attest(ctx.shipment, chainHash, { passed: {} });
      }
      const full = await program.account.shipment.fetch(ctx.shipment);
      assert.equal(full.attestations.length, 8);

      try {
        await attest(ctx.shipment, chainHash, { passed: {} });
        assert.fail("expected AttestationCapacityExceeded");
      } catch (err: any) {
        assert.match(err.toString(), /AttestationCapacityExceeded/);
      }
    });

    it("test_attestation_records_chain_hash: stored chain_hash matches the shipment's chain_hash", async () => {
      const ctx = await setupClosedShipment(3);
      const acct = await program.account.shipment.fetch(ctx.shipment);
      const chainHash = acct.chainHash;

      await attest(ctx.shipment, chainHash, { inconclusive: {} });

      const after = await program.account.shipment.fetch(ctx.shipment);
      assert.deepEqual(after.attestations[0].chainHashAtVerification, after.chainHash);
      assert.deepEqual(after.attestations[0].chainHashAtVerification, chainHash);
    });
  });
});
