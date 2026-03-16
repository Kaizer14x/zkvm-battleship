/// Integration tests for Round 0 — validate_board ZK proof
///
/// Phase 6 requirements:
///   • valid board   → proof succeeds and commitment is a 32-byte hash
///   • invalid board → prove() panics / returns an error (guest assertions fail)
use battleship_core::{BoardCommitInput, BoardCommitOutput, Orientation, Ship, ShipType};
use methods::{VALIDATE_BOARD_ELF, VALIDATE_BOARD_ID};
use risc0_zkvm::{default_prover, ExecutorEnv};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A known-valid Battleship board layout used in all positive tests.
///
///  Carrier    (5) — row 0, col 0, Horizontal  → cells (0,0)...(0,4)
///  Battleship (4) — row 2, col 0, Horizontal  → cells (2,0)...(2,3)
///  Cruiser    (3) — row 4, col 0, Horizontal  → cells (4,0)...(4,2)
///  Submarine  (3) — row 6, col 0, Horizontal  → cells (6,0)...(6,2)
///  Destroyer  (2) — row 8, col 0, Horizontal  → cells (8,0)...(8,1)
///
/// Total cells = 5+4+3+3+2 = 17.  No overlaps.  All within bounds.
fn valid_ships() -> [Ship; 5] {
    [
        Ship {
            ship_type: ShipType::Carrier,
            row: 0,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Battleship,
            row: 2,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Cruiser,
            row: 4,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Submarine,
            row: 6,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Destroyer,
            row: 8,
            col: 0,
            orientation: Orientation::Horizontal,
        },
    ]
}

fn zero_blinding() -> [u8; 32] {
    [0u8; 32]
}

/// Run the prover for the given input and return the decoded journal output
/// on success.  Panics (via expect) if proving or verification fails.
fn prove_and_verify(input: &BoardCommitInput) -> BoardCommitOutput {
    let env = ExecutorEnv::builder()
        .write(input)
        .expect("serialise input")
        .build()
        .expect("build ExecutorEnv");

    let receipt = default_prover()
        .prove(env, VALIDATE_BOARD_ELF)
        .expect("prove() failed")
        .receipt;

    // Cryptographic receipt verification — confirms the STARK is valid and
    // the IMAGE_ID matches our exact guest binary.
    receipt
        .verify(VALIDATE_BOARD_ID)
        .expect("receipt.verify() failed");

    receipt
        .journal
        .decode::<BoardCommitOutput>()
        .expect("decode journal")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// A valid board produces a proof whose journal contains a 32-byte commitment.
#[test]
fn valid_board_produces_commitment() {
    let input = BoardCommitInput {
        ships: valid_ships(),
        blinding: zero_blinding(),
    };

    let output = prove_and_verify(&input);

    // The commitment must be non-zero (extremely unlikely to be all-zeros for
    // valid input + zero blinding).
    assert_ne!(
        output.commitment, [0u8; 32],
        "commitment should not be all zeros"
    );
}

/// The commitment is deterministic: same ships + same blinding → same hash.
#[test]
fn commitment_is_deterministic() {
    let input = BoardCommitInput {
        ships: valid_ships(),
        blinding: zero_blinding(),
    };

    let out1 = prove_and_verify(&input);
    let out2 = prove_and_verify(&input);

    assert_eq!(
        out1.commitment, out2.commitment,
        "same input must produce the same commitment"
    );
}

/// A different blinding factor produces a different commitment, even for the
/// same board.  This is the purpose of the blinding salt.
#[test]
fn different_blinding_different_commitment() {
    let blinding_a = [0u8; 32];
    let mut blinding_b = [0u8; 32];
    blinding_b[0] = 1;

    let out_a = prove_and_verify(&BoardCommitInput {
        ships: valid_ships(),
        blinding: blinding_a,
    });
    let out_b = prove_and_verify(&BoardCommitInput {
        ships: valid_ships(),
        blinding: blinding_b,
    });

    assert_ne!(
        out_a.commitment, out_b.commitment,
        "different blinding must produce different commitments"
    );
}

/// An invalid board (overlapping ships) must NOT produce a valid proof.
/// The guest asserts no overlap; if the assertion fires, prove() returns an
/// error.  We verify the error is surfaced correctly.
#[test]
fn overlapping_ships_fail_to_prove() {
    // Carrier and Battleship both start at (0, 0) — guaranteed overlap.
    let bad_ships = [
        Ship {
            ship_type: ShipType::Carrier,
            row: 0,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Battleship,
            row: 0,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Cruiser,
            row: 4,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Submarine,
            row: 6,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Destroyer,
            row: 8,
            col: 0,
            orientation: Orientation::Horizontal,
        },
    ];

    let env = ExecutorEnv::builder()
        .write(&BoardCommitInput {
            ships: bad_ships,
            blinding: zero_blinding(),
        })
        .expect("serialise input")
        .build()
        .expect("build ExecutorEnv");

    let result = default_prover().prove(env, VALIDATE_BOARD_ELF);
    assert!(
        result.is_err(),
        "prove() should fail for an overlapping board"
    );
}

/// A ship that extends past the grid edge must NOT produce a valid proof.
#[test]
fn out_of_bounds_ship_fails_to_prove() {
    // Carrier at col 8, horizontal → needs cols 8..13 but grid is 0..10.
    let bad_ships = [
        Ship {
            ship_type: ShipType::Carrier,
            row: 0,
            col: 8,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Battleship,
            row: 2,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Cruiser,
            row: 4,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Submarine,
            row: 6,
            col: 0,
            orientation: Orientation::Horizontal,
        },
        Ship {
            ship_type: ShipType::Destroyer,
            row: 8,
            col: 0,
            orientation: Orientation::Horizontal,
        },
    ];

    let env = ExecutorEnv::builder()
        .write(&BoardCommitInput {
            ships: bad_ships,
            blinding: zero_blinding(),
        })
        .expect("serialise input")
        .build()
        .expect("build ExecutorEnv");

    let result = default_prover().prove(env, VALIDATE_BOARD_ELF);
    assert!(
        result.is_err(),
        "prove() should fail for a ship out of bounds"
    );
}
