/// Unit-level tests for the GameStore engine logic.
///
/// These tests exercise game state transitions (attack log, transcript,
/// hit-log derivation, role swapping, game-over detection) without
/// invoking the zkVM prover.  They run in milliseconds.
use battleship_core::{AttackResult, Orientation, Ship, ShipType};

#[allow(dead_code)]
#[path = "../src/storage.rs"]
mod storage;

use storage::GameStore;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn setup_store() -> GameStore {
    let mut store = GameStore::new();
    let ships = [
        Ship { ship_type: ShipType::Carrier,    row: 0, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Battleship, row: 2, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Cruiser,    row: 4, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Submarine,  row: 6, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Destroyer,  row: 8, col: 0, orientation: Orientation::Horizontal },
    ];
    store.set_ships(0, ships);
    store.set_ships(1, ships);
    store.set_commitment(0, [1u8; 32]);
    store.set_commitment(1, [2u8; 32]);
    store.set_blinding(0, [10u8; 32]);
    store.set_blinding(1, [20u8; 32]);
    store.round = 1;
    store
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn duplicate_attack_rejected() {
    let mut store = setup_store();

    store.record_attack((3, 3));
    assert!(store.is_already_attacked((3, 3)));
    assert!(!store.is_already_attacked((3, 4)));

    store.record_attack((5, 7));
    assert!(store.is_already_attacked((5, 7)));
    assert!(!store.is_already_attacked((7, 5)));
}

#[test]
fn role_swapping() {
    let mut store = setup_store();

    assert_eq!(store.active_attacker, 0);
    assert_eq!(store.active_defender(), 1);

    store.swap_roles();
    assert_eq!(store.active_attacker, 1);
    assert_eq!(store.active_defender(), 0);

    store.swap_roles();
    assert_eq!(store.active_attacker, 0);
    assert_eq!(store.active_defender(), 1);
}

#[test]
fn hit_log_filters_by_defender() {
    let mut store = setup_store();

    // Transcript index 0 (even) → P0 attacked, P1 defended → HIT on P1
    store.record_result((0, 0), AttackResult::Hit);
    // Transcript index 1 (odd) → P1 attacked, P0 defended → MISS on P0
    store.record_result((5, 5), AttackResult::Miss);
    // Transcript index 2 (even) → P0 attacked, P1 defended → HIT on P1
    store.record_result((0, 1), AttackResult::Hit);
    // Transcript index 3 (odd) → P1 attacked, P0 defended → HIT on P0
    store.record_result((9, 9), AttackResult::Hit);

    // Player 1 was hit at indices 0 and 2
    assert_eq!(store.hit_log_for_defender(1), vec![(0, 0), (0, 1)]);
    assert_eq!(store.hit_count_for_defender(1), 2);

    // Player 0 was hit at index 3 only (index 1 was a miss)
    assert_eq!(store.hit_log_for_defender(0), vec![(9, 9)]);
    assert_eq!(store.hit_count_for_defender(0), 1);
}

#[test]
fn transcript_grows_correctly() {
    let mut store = setup_store();

    assert_eq!(store.transcript.len(), 0);

    store.record_result((0, 0), AttackResult::Hit);
    assert_eq!(store.transcript.len(), 1);
    assert_eq!(store.transcript[0].coord, (0, 0));
    assert_eq!(store.transcript[0].result, AttackResult::Hit);

    store.record_result((1, 1), AttackResult::Miss);
    assert_eq!(store.transcript.len(), 2);
    assert_eq!(store.transcript[1].coord, (1, 1));
    assert_eq!(store.transcript[1].result, AttackResult::Miss);
}

#[test]
fn sunk_ship_recording() {
    let mut store = setup_store();

    assert!(store.sunk_ships[0].is_empty());
    assert!(store.sunk_ships[1].is_empty());

    store.record_sunk(1, 0); // P1's Carrier sunk
    store.record_sunk(1, 4); // P1's Destroyer sunk
    assert_eq!(store.sunk_ships[1], vec![0, 4]);
    assert!(store.sunk_ships[0].is_empty());

    store.record_sunk(0, 2); // P0's Cruiser sunk
    assert_eq!(store.sunk_ships[0], vec![2]);
}

#[test]
fn no_game_over_before_17_hits() {
    let mut store = setup_store();

    assert!(store.is_game_over().is_none());

    // 16 hits against P1 (even indices) with 15 miss fillers (odd indices)
    for i in 0..16u8 {
        store.record_result((i / 10, i % 10), AttackResult::Hit);
        if i < 15 {
            store.record_result((9, i), AttackResult::Miss);
        }
    }

    assert_eq!(store.hit_count_for_defender(1), 16);
    assert!(
        store.is_game_over().is_none(),
        "game should not be over at 16 hits"
    );
}

#[test]
fn game_over_at_17_hits() {
    let mut store = setup_store();

    // All 17 ship cells of the valid board, targeted in order
    let p0_hits: [(u8, u8); 17] = [
        (0, 0), (0, 1), (0, 2), (0, 3), (0, 4), // Carrier
        (2, 0), (2, 1), (2, 2), (2, 3),           // Battleship
        (4, 0), (4, 1), (4, 2),                    // Cruiser
        (6, 0), (6, 1), (6, 2),                    // Submarine
        (8, 0), (8, 1),                            // Destroyer
    ];

    // P1 misses on distinct cells each time
    let p1_misses: [(u8, u8); 16] = [
        (9, 9), (9, 8), (9, 7), (9, 6), (9, 5),
        (7, 9), (7, 8), (7, 7), (7, 6), (7, 5),
        (5, 9), (5, 8), (5, 7), (5, 6), (5, 5),
        (3, 9),
    ];

    for i in 0..17 {
        // Even transcript index → P0 attacks P1
        store.record_result(p0_hits[i], AttackResult::Hit);
        if i < 16 {
            // Odd transcript index → P1 attacks P0
            store.record_result(p1_misses[i], AttackResult::Miss);
        }
    }

    assert_eq!(store.hit_count_for_defender(1), 17);
    assert_eq!(
        store.is_game_over(),
        Some(0),
        "Player 0 should win after 17 hits on Player 1"
    );
}

#[test]
fn initial_state_is_clean() {
    let store = GameStore::new();

    assert_eq!(store.round, 0);
    assert_eq!(store.active_attacker, 0);
    assert!(store.attack_log.is_empty());
    assert!(store.transcript.is_empty());
    assert!(store.sunk_ships[0].is_empty());
    assert!(store.sunk_ships[1].is_empty());
    assert!(store.commitments[0].is_none());
    assert!(store.commitments[1].is_none());
    assert!(store.blindings[0].is_none());
    assert!(store.blindings[1].is_none());
    assert!(store.players[0].ships.is_none());
    assert!(store.players[1].ships.is_none());
}
