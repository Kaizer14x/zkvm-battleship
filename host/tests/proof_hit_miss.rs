/// Integration tests for the hit/miss ZK proof.
///
/// Each test runs the real RISC-V guest program through the zkVM prover
/// and verifies the cryptographic receipt and journal output.
mod common;

use battleship_core::{AttackResult, HitMissInput, HitMissOutput};
use methods::{HIT_MISS_ELF, HIT_MISS_ID};
use risc0_zkvm::{default_prover, ExecutorEnv};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn prove_hit_miss(input: &HitMissInput) -> HitMissOutput {
    let env = ExecutorEnv::builder()
        .write(input)
        .expect("serialise HitMissInput")
        .build()
        .expect("build ExecutorEnv");

    let receipt = default_prover()
        .prove(env, HIT_MISS_ELF)
        .expect("hit_miss prove() failed")
        .receipt;

    receipt
        .verify(HIT_MISS_ID)
        .expect("hit_miss receipt verification failed");

    receipt.journal.decode().expect("decode HitMissOutput")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Attacking an occupied cell produces a verified HIT result.
#[test]
fn hit_on_occupied_cell() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();
    let commitment = common::prove_board_commitment(&ships, &blinding);

    // (0, 2) is occupied by the Carrier → (0,0)…(0,4)
    let input = HitMissInput {
        ships,
        blinding,
        attack_coord: (0, 2),
        round_number: 1,
    };

    let output = prove_hit_miss(&input);

    assert_eq!(output.result, AttackResult::Hit);
    assert_eq!(output.attack_coord, (0, 2));
    assert_eq!(output.commitment, commitment);
    assert_eq!(output.round_number, 1);
}

/// Attacking an empty cell produces a verified MISS result.
#[test]
fn miss_on_empty_cell() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();
    let commitment = common::prove_board_commitment(&ships, &blinding);

    // (1, 0) is an empty row between Carrier and Battleship
    let input = HitMissInput {
        ships,
        blinding,
        attack_coord: (1, 0),
        round_number: 1,
    };

    let output = prove_hit_miss(&input);

    assert_eq!(output.result, AttackResult::Miss);
    assert_eq!(output.attack_coord, (1, 0));
    assert_eq!(output.commitment, commitment);
}

/// The commitment output by hit_miss must exactly match the commitment
/// produced by validate_board for the same ships+blinding.
/// This is the chain-of-trust across rounds.
#[test]
fn commitment_chains_with_round0() {
    let ships = common::valid_ships();
    let blinding = common::zero_blinding();

    // Round 0: board commitment
    let round0_commitment = common::prove_board_commitment(&ships, &blinding);

    // Round 1: hit/miss proof
    let input = HitMissInput {
        ships,
        blinding,
        attack_coord: (0, 0),
        round_number: 1,
    };

    let output = prove_hit_miss(&input);

    assert_eq!(
        output.commitment, round0_commitment,
        "hit_miss commitment must match validate_board commitment"
    );
}

/// Different blinding factors produce different commitments even for the
/// same board and same attack — demonstrating the blinding salt works.
#[test]
fn different_blinding_different_commitment() {
    let ships = common::valid_ships();

    let input_a = HitMissInput {
        ships,
        blinding: [0u8; 32],
        attack_coord: (0, 0),
        round_number: 1,
    };

    let mut blinding_b = [0u8; 32];
    blinding_b[0] = 1;
    let input_b = HitMissInput {
        ships,
        blinding: blinding_b,
        attack_coord: (0, 0),
        round_number: 1,
    };

    let out_a = prove_hit_miss(&input_a);
    let out_b = prove_hit_miss(&input_b);

    assert_ne!(
        out_a.commitment, out_b.commitment,
        "different blinding must yield different commitments"
    );
    // Both attacks land on the Carrier → both should be HIT
    assert_eq!(out_a.result, AttackResult::Hit);
    assert_eq!(out_b.result, AttackResult::Hit);
}
