/// Shared helpers for integration tests.
///
/// Provides a canonical valid board layout and a helper to generate
/// the board commitment via the validate_board ZK proof.
use battleship_core::{BoardCommitInput, BoardCommitOutput, Orientation, Ship, ShipType};
use methods::{VALIDATE_BOARD_ELF, VALIDATE_BOARD_ID};
use risc0_zkvm::{default_prover, ExecutorEnv};

/// A known-valid board layout used across all tests.
///
///  Carrier    (5) — row 0, col 0, Horizontal  → (0,0)…(0,4)
///  Battleship (4) — row 2, col 0, Horizontal  → (2,0)…(2,3)
///  Cruiser    (3) — row 4, col 0, Horizontal  → (4,0)…(4,2)
///  Submarine  (3) — row 6, col 0, Horizontal  → (6,0)…(6,2)
///  Destroyer  (2) — row 8, col 0, Horizontal  → (8,0)…(8,1)
pub fn valid_ships() -> [Ship; 5] {
    [
        Ship { ship_type: ShipType::Carrier,    row: 0, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Battleship, row: 2, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Cruiser,    row: 4, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Submarine,  row: 6, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Destroyer,  row: 8, col: 0, orientation: Orientation::Horizontal },
    ]
}

pub fn zero_blinding() -> [u8; 32] {
    [0u8; 32]
}

/// Run the validate_board ZK proof and return the 32-byte commitment.
/// This mirrors Round 0 and is used by later proof tests to chain
/// commitments across rounds.
pub fn prove_board_commitment(ships: &[Ship; 5], blinding: &[u8; 32]) -> [u8; 32] {
    let input = BoardCommitInput {
        ships: *ships,
        blinding: *blinding,
    };
    let env = ExecutorEnv::builder()
        .write(&input)
        .expect("serialise BoardCommitInput")
        .build()
        .expect("build ExecutorEnv");

    let receipt = default_prover()
        .prove(env, VALIDATE_BOARD_ELF)
        .expect("validate_board prove() failed")
        .receipt;

    receipt
        .verify(VALIDATE_BOARD_ID)
        .expect("validate_board receipt verification failed");

    let output: BoardCommitOutput = receipt.journal.decode().expect("decode BoardCommitOutput");
    output.commitment
}
