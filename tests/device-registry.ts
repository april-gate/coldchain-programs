import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { DeviceRegistry } from "../target/types/device_registry";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
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
    crypto.randomBytes(6).copy(id, 2); // 6 random middle bytes
    id[8] = 0xee;
    // bytes 9..32 stay zero
    return Array.from(id);
  }

  /** Mock compressed P-256 public key (33 bytes). For tests; not validated on-chain. */
  function makePubkey(): number[] {
    const pk = Buffer.alloc(33);
    pk[0] = 0x02; // compressed prefix
    crypto.randomBytes(32).copy(pk, 1);
    return Array.from(pk);
  }

  /** Random 32-byte commitment for proof submissions. */
  function makeCommitment(): number[] {
    return Array.from(crypto.randomBytes(32));
  }

  function devicePda(deviceId: number[]): PublicKey {
    const [pda] = PublicKey.findProgramAddressSync(
      [Buffer.from("device"), Buffer.from(deviceId)],
      program.programId
    );
    return pda;
  }

  function shipmentPda(authority: PublicKey, nonce: anchor.BN): PublicKey {
    const [pda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("shipment"),
        authority.toBuffer(),
        nonce.toArrayLike(Buffer, "le", 8),
      ],
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

  function proofPda(assignment: PublicKey, sequence: number): PublicKey {
    const seqBuf = Buffer.alloc(4);
    seqBuf.writeUInt32LE(sequence, 0);
    const [pda] = PublicKey.findProgramAddressSync(
      [Buffer.from("proof"), assignment.toBuffer(), seqBuf],
      program.programId
    );
    return pda;
  }

  // Tests ─────────────────────────────────────────────────────────────────────

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

  it("creates a shipment", async () => {
    const nonce = new BN(1);
    const shipment = shipmentPda(authority.publicKey, nonce);

    await program.methods
      .createShipment(nonce)
      .accounts({
        shipment,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const acct = await program.account.shipment.fetch(shipment);
    assert.deepEqual(acct.status, { created: {} });
    assert.equal(acct.nonce.toNumber(), 1);
    assert.equal(acct.proofCount, 0);
  });

  it("transitions shipment Created → InTransit → Delivered → Closed", async () => {
    const nonce = new BN(2);
    const shipment = shipmentPda(authority.publicKey, nonce);

    await program.methods
      .createShipment(nonce)
      .accounts({
        shipment,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const transitions = [
      { inTransit: {} },
      { delivered: {} },
      { closed: {} },
    ];
    for (const status of transitions) {
      await program.methods
        .updateShipmentStatus(status as any)
        .accounts({ shipment, authority: authority.publicKey })
        .rpc();
    }

    const acct = await program.account.shipment.fetch(shipment);
    assert.deepEqual(acct.status, { closed: {} });
  });

  it("rejects invalid status transitions", async () => {
    const nonce = new BN(3);
    const shipment = shipmentPda(authority.publicKey, nonce);

    await program.methods
      .createShipment(nonce)
      .accounts({
        shipment,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Created → Delivered should fail (must go through InTransit)
    try {
      await program.methods
        .updateShipmentStatus({ delivered: {} } as any)
        .accounts({ shipment, authority: authority.publicKey })
        .rpc();
      assert.fail("expected InvalidStatusTransition");
    } catch (err: any) {
      assert.match(err.toString(), /InvalidStatusTransition/);
    }
  });

  it("runs the full lifecycle: register → create → assign → submit proof → end → reassign", async () => {
    // Register the device
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

    // Create shipment A
    const nonceA = new BN(100);
    const shipmentA = shipmentPda(authority.publicKey, nonceA);
    await program.methods
      .createShipment(nonceA)
      .accounts({
        shipment: shipmentA,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Assign device to shipment A (sequence 0)
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

    // Submit two proofs against assignment 0
    for (let i = 0; i < 2; i++) {
      const proof = proofPda(assignment0, i);
      await program.methods
        .submitProof(makeCommitment())
        .accounts({
          assignment: assignment0,
          shipment: shipmentA,
          proof,
          submitter: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    }

    let assignAcct = await program.account.deviceAssignment.fetch(assignment0);
    let shipmentAcct = await program.account.shipment.fetch(shipmentA);
    assert.equal(assignAcct.proofCount, 2);
    assert.equal(shipmentAcct.proofCount, 2);

    // Cannot reassign while still active
    const nonceB = new BN(101);
    const shipmentB = shipmentPda(authority.publicKey, nonceB);
    await program.methods
      .createShipment(nonceB)
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

    // End the first assignment
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
    assignAcct = await program.account.deviceAssignment.fetch(assignment0);
    assert.notEqual(assignAcct.endedAt.toNumber(), 0);

    // Reassign to shipment B (sequence 1)
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

    // Verify history: assignment 0 ended, assignment 1 active
    const hist0 = await program.account.deviceAssignment.fetch(assignment0);
    const hist1 = await program.account.deviceAssignment.fetch(assignment1);
    assert.notEqual(hist0.endedAt.toNumber(), 0);
    assert.equal(hist1.endedAt.toNumber(), 0);
    assert.ok(hist0.shipment.equals(shipmentA));
    assert.ok(hist1.shipment.equals(shipmentB));
  });

  it("getProgramAccounts can find all assignments for a shipment", async () => {
    // This is the killer query for the insurance use case:
    // "show me every device that was ever on this shipment"
    const nonce = new BN(200);
    const shipment = shipmentPda(authority.publicKey, nonce);
    await program.methods
      .createShipment(nonce)
      .accounts({
        shipment,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Register two devices and assign both to this shipment
    for (let i = 0; i < 2; i++) {
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

    // Filter assignments by shipment field. Layout offset = 8 (discriminator) + 32 (device).
    const assignments = await program.account.deviceAssignment.all([
      {
        memcmp: {
          offset: 8 + 32,
          bytes: shipment.toBase58(),
        },
      },
    ]);

    assert.equal(assignments.length, 2);
    for (const a of assignments) {
      assert.ok(a.account.shipment.equals(shipment));
    }
  });
});
