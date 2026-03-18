/// Integration tests for the ship-sunk and no-ship-sunk ZK proofs.
///
/// Each test runs the real RISC-V guest program through the zkVM prover
/// and verifies the cryptographic receipt and journal output.
mod common;

use battleship_core::{NoShipSunkInput, NoShipSunkOutput, ShipSunkInput, ShipSunkOutput};
use methods::{NO_SHIP_SUNK_ELF, NO_SHIP_SUNK_ID, SHIP_SUNK_ELF, SHIP_SUNK_ID};
use risc0_zkvm::{default_prover, ExecutorEnv};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn prove_ship_sunk(input: &ShipSunkInput) -> ShipSunkOutput {
    let env = ExecutorEnv::builder()
        .write(input)
        .expect("serialise ShipSunkInput")
        .build()
        .expect("build ExecutorEnv");

    let receipt = default_prover()
        .prove(env, SHIP_SUNK_ELF)
        .expect("ship_sunk prove() failed")
        .receipt;

    receipt
        .verify(SHIP_SUNK_ID)
        .expect("ship_sunk receipt verification failed");

    receipt.journal.decode().expect("decode ShipSunkOutput")
}

fn prove_no_ship_sunk(input: &NoShipSunkInput) -> NoShipSunkOutput {
    let env = ExecutorEnv::builder()
        .write(input)
        .expect("serialise NoShipSunkInput")
        .build()
        .expect("build ExecutorEnv");

    let receipt = default_prover()
        .prove(env, NO_SHIP_SUNK_ELF)
        .expect("no_ship_sunk prove() failed")
        .receipt;

    receipt
        .verify(NO_SHIP_SUNK_ID)
        .expect("no_ship_sunk receipt verification failed");

    receipt
        .journal
        .decode()
        .expect("decode NoShipSunkOutput")
}

// ===========================================================================
// Ship Sunk proof tests
// ===========================================================================

/// Destroyer (index 4) at (8,0)-(8,1) — all cells hit → proof succeeds.
#[test]
fn ship_sunk_all_cells_hit() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();
    let commitment = common::prove_board_commitment(&ships, &blinding);

    let hit_log = vec![(8, 0), (8, 1)];
    let hit_indices = vec![0, 1];

    let input = ShipSunkInput {
        ships,
        blinding,
        sunk_ship_index: 4,
        hit_log,
        hit_indices,
    };

    let output = prove_ship_sunk(&input);

    assert_eq!(output.commitment, commitment);
    assert_eq!(output.ship_index, 4);
    assert_eq!(output.transcript_length, 2);
}

/// Cruiser (index 2) at (4,0)-(4,2) — all cells present among noise entries.
#[test]
fn ship_sunk_with_noise_in_hit_log() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();
    let commitment = common::prove_board_commitment(&ships, &blinding);

    // hit_log has extra entries mixed in
    let hit_log = vec![
        (1, 1), // noise  (index 0)
        (4, 0), // cruiser cell 0 (index 1)
        (3, 3), // noise  (index 2)
        (4, 1), // cruiser cell 1 (index 3)
        (4, 2), // cruiser cell 2 (index 4)
        (7, 7), // noise  (index 5)
    ];
    let hit_indices = vec![1, 3, 4];

    let input = ShipSunkInput {
        ships,
        blinding,
        sunk_ship_index: 2,
        hit_log,
        hit_indices,
    };

    let output = prove_ship_sunk(&input);

    assert_eq!(output.commitment, commitment);
    assert_eq!(output.ship_index, 2);
    assert_eq!(output.transcript_length, 6);
}

/// Carrier (index 0, length 5) — prove the longest ship can be sunk.
#[test]
fn ship_sunk_carrier() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();
    let commitment = common::prove_board_commitment(&ships, &blinding);

    let hit_log = vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)];
    let hit_indices = vec![0, 1, 2, 3, 4];

    let input = ShipSunkInput {
        ships,
        blinding,
        sunk_ship_index: 0,
        hit_log,
        hit_indices,
    };

    let output = prove_ship_sunk(&input);

    assert_eq!(output.commitment, commitment);
    assert_eq!(output.ship_index, 0);
    assert_eq!(output.transcript_length, 5);
}

/// Duplicate hit_indices → the guest asserts distinctness and the proof fails.
#[test]
fn ship_sunk_non_distinct_indices_fails() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();

    // Destroyer at (8,0)-(8,1).  Both cells are hit, but we provide
    // the same index for both → the guest must reject.
    let hit_log = vec![(8, 0), (8, 1)];
    let hit_indices = vec![0, 0]; // duplicate!

    let input = ShipSunkInput {
        ships,
        blinding,
        sunk_ship_index: 4,
        hit_log,
        hit_indices,
    };

    let env = ExecutorEnv::builder()
        .write(&input)
        .expect("serialise")
        .build()
        .expect("build env");

    let result = default_prover().prove(env, SHIP_SUNK_ELF);
    assert!(
        result.is_err(),
        "duplicate hit_indices should cause the proof to fail"
    );
}

// ===========================================================================
// No Ship Sunk proof tests
// ===========================================================================

/// No hits at all — every ship trivially has a surviving cell.
#[test]
fn no_ship_sunk_empty_hit_log() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();
    let commitment = common::prove_board_commitment(&ships, &blinding);

    let input = NoShipSunkInput {
        ships,
        blinding,
        surviving_cell_indices: [0, 0, 0, 0, 0],
        hit_log: vec![],
    };

    let output = prove_no_ship_sunk(&input);

    assert_eq!(output.commitment, commitment);
    assert_eq!(output.transcript_length, 0);
}

/// Each ship has been partially hit (first cell only) — surviving cells exist.
#[test]
fn no_ship_sunk_partial_hits() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();
    let commitment = common::prove_board_commitment(&ships, &blinding);

    // Hit only the first cell of each ship
    let hit_log = vec![(0, 0), (2, 0), (4, 0), (6, 0), (8, 0)];

    // surviving_cell_indices: cell index 1 of each ship is still alive
    // Carrier[1]=(0,1), Battleship[1]=(2,1), Cruiser[1]=(4,1),
    // Submarine[1]=(6,1), Destroyer[1]=(8,1)
    let surviving_cell_indices = [1u8, 1, 1, 1, 1];

    let input = NoShipSunkInput {
        ships,
        blinding,
        surviving_cell_indices,
        hit_log,
    };

    let output = prove_no_ship_sunk(&input);

    assert_eq!(output.commitment, commitment);
    assert_eq!(output.transcript_length, 5);
}

/// Claimed survivor cell is actually in the hit log → proof must fail.
/// All Carrier cells are hit; claiming cell 0 of Carrier as survivor is false.
#[test]
fn no_ship_sunk_false_survivor_fails() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();

    // All 5 Carrier cells hit
    let hit_log = vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)];

    // Claim cell index 0 of Carrier as surviving — but (0,0) IS in hit_log
    let input = NoShipSunkInput {
        ships,
        blinding,
        surviving_cell_indices: [0, 0, 0, 0, 0],
        hit_log,
    };

    let env = ExecutorEnv::builder()
        .write(&input)
        .expect("serialise")
        .build()
        .expect("build env");

    let result = default_prover().prove(env, NO_SHIP_SUNK_ELF);
    assert!(
        result.is_err(),
        "false survivor (cell in hit_log) should cause proof to fail"
    );
}
