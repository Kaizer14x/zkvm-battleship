/// Comprehensive end-to-end game scenario.
///
/// Drives a GameStore from board setup through 33 rounds to a terminal
/// state, validating role swaps, transcript growth, hit-log correctness,
/// automatic sinking detection, duplicate prevention, and winner
/// determination.
///
/// No zkVM prover calls — this tests the game engine logic only (fast).
use battleship_core::{AttackResult, Orientation, Ship, ShipType};

#[allow(dead_code)]
#[path = "../src/storage.rs"]
mod storage;

use storage::GameStore;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Board for Player 0 (ships spread across even rows).
fn ships_p0() -> [Ship; 5] {
    [
        Ship { ship_type: ShipType::Carrier,    row: 0, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Battleship, row: 2, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Cruiser,    row: 4, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Submarine,  row: 6, col: 0, orientation: Orientation::Horizontal },
        Ship { ship_type: ShipType::Destroyer,  row: 8, col: 0, orientation: Orientation::Horizontal },
    ]
}

/// Board for Player 1 (ships along the right edge, vertical).
fn ships_p1() -> [Ship; 5] {
    [
        Ship { ship_type: ShipType::Carrier,    row: 0, col: 9, orientation: Orientation::Vertical },
        Ship { ship_type: ShipType::Battleship, row: 0, col: 8, orientation: Orientation::Vertical },
        Ship { ship_type: ShipType::Cruiser,    row: 0, col: 7, orientation: Orientation::Vertical },
        Ship { ship_type: ShipType::Submarine,  row: 5, col: 7, orientation: Orientation::Vertical },
        Ship { ship_type: ShipType::Destroyer,  row: 5, col: 8, orientation: Orientation::Vertical },
    ]
}

/// Collect all 17 cell coordinates for a fleet.
fn all_cells(ships: &[Ship; 5]) -> Vec<(u8, u8)> {
    ships.iter().flat_map(|s| s.cells()).collect()
}

/// Replicate the engine's auto-sinking detection logic.
/// Returns Some(ship_index) if any unsunk ship now has all cells hit.
fn find_newly_sunk(ships: &[Ship; 5], hit_log: &[(u8, u8)], already_sunk: &[u8]) -> Option<u8> {
    for (i, ship) in ships.iter().enumerate() {
        if already_sunk.contains(&(i as u8)) {
            continue;
        }
        if ship.cells().all(|cell| hit_log.contains(&cell)) {
            return Some(i as u8);
        }
    }
    None
}

/// Determine expected result for an attack against a set of ships.
fn expected_result(coord: (u8, u8), ships: &[Ship; 5]) -> AttackResult {
    for ship in ships {
        if ship.cells().any(|c| c == coord) {
            return AttackResult::Hit;
        }
    }
    AttackResult::Miss
}

/// Run one round: attack, record result, optionally detect sinking, check
/// game over, swap roles, increment round.
/// Returns Some(winner_id) if the game ends this round.
fn play_scripted_round(
    store: &mut GameStore,
    attack: (u8, u8),
    result: AttackResult,
    defender_ships: &[Ship; 5],
) -> Option<usize> {
    let defender = store.active_defender();

    assert!(
        !store.is_already_attacked(attack),
        "duplicate attack ({}, {}) in round {}",
        attack.0,
        attack.1,
        store.round,
    );

    store.record_attack(attack);
    store.record_result(attack, result);

    // Automatic sinking detection (mirrors logic.rs engine behaviour).
    if result == AttackResult::Hit {
        let hit_log = store.hit_log_for_defender(defender);
        if let Some(sunk_idx) = find_newly_sunk(defender_ships, &hit_log, &store.sunk_ships[defender]) {
            store.record_sunk(defender, sunk_idx);
        }
    }

    if let Some(winner) = store.is_game_over() {
        return Some(winner);
    }

    store.swap_roles();
    store.round += 1;
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Full game: Player 0 sinks Player 1's entire fleet while Player 1 misses
/// every shot.  Verifies transcript, hit counts, sinking order, and winner.
#[test]
fn full_game_player_0_wins() {
    let mut store = GameStore::new();

    let sp0 = ships_p0();
    let sp1 = ships_p1();

    store.set_ships(0, sp0);
    store.set_ships(1, sp1);
    store.set_commitment(0, [1u8; 32]);
    store.set_commitment(1, [2u8; 32]);
    store.set_blinding(0, [10u8; 32]);
    store.set_blinding(1, [20u8; 32]);
    store.round = 1;

    // P0 will hit every cell of P1's fleet in order.
    let p0_attacks = all_cells(&sp1);
    assert_eq!(p0_attacks.len(), 17);

    // P1 will miss on distinct empty cells that are:
    //  - NOT P0 ship cells (so they are misses on P0's board)
    //  - NOT P1 ship cells (so they don't collide with P0's attacks in the shared attack_log)
    let p1_misses: Vec<(u8, u8)> = vec![
        (1, 0), (1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6),  // row 1 (empty for both)
        (3, 0), (3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6),  // row 3 (empty for both)
        (5, 0), (5, 1),                                            // row 5, cols 0-1
    ];

    let mut winner: Option<usize> = None;
    let mut rounds_played = 0;

    for i in 0..17 {
        // P0 attacks P1 (hit)
        let result = expected_result(p0_attacks[i], &sp1);
        assert_eq!(result, AttackResult::Hit);
        winner = play_scripted_round(&mut store, p0_attacks[i], result, &sp1);
        rounds_played += 1;
        if winner.is_some() {
            break;
        }

        // P1 attacks P0 (miss)
        if i < 16 {
            let result = expected_result(p1_misses[i], &sp0);
            assert_eq!(result, AttackResult::Miss);
            winner = play_scripted_round(&mut store, p1_misses[i], result, &sp0);
            rounds_played += 1;
            if winner.is_some() {
                break;
            }
        }
    }

    // --- Assertions ---
    assert_eq!(winner, Some(0), "Player 0 should win");
    assert_eq!(rounds_played, 33, "17 P0 attacks + 16 P1 attacks = 33 rounds");
    assert_eq!(store.hit_count_for_defender(1), 17);
    assert_eq!(store.hit_count_for_defender(0), 0);

    // All 5 of P1's ships should be recorded as sunk.
    assert_eq!(store.sunk_ships[1].len(), 5, "all 5 ships of P1 should be sunk");
    assert!(store.sunk_ships[0].is_empty(), "P0 should have no sunk ships");
}

/// Sinking is detected at the exact right round — not before, not after.
#[test]
fn sinking_detected_at_correct_moment() {
    let mut store = GameStore::new();

    let sp0 = ships_p0();
    let sp1 = ships_p1();

    store.set_ships(0, sp0);
    store.set_ships(1, sp1);
    store.set_commitment(0, [1u8; 32]);
    store.set_commitment(1, [2u8; 32]);
    store.set_blinding(0, [10u8; 32]);
    store.set_blinding(1, [20u8; 32]);
    store.round = 1;

    // P1's Destroyer is at (5,8)-(6,8), the shortest ship.
    // Target it first: 2 hits should sink it.
    let destroyer_cells: Vec<_> = sp1[4].cells().collect();
    assert_eq!(destroyer_cells.len(), 2);

    // Miss filler for P1.
    let p1_miss = (9, 9);

    // Hit 1: first cell of Destroyer — no sinking yet.
    play_scripted_round(&mut store, destroyer_cells[0], AttackResult::Hit, &sp1);
    assert!(
        store.sunk_ships[1].is_empty(),
        "no ship sunk after first hit"
    );

    // P1 misses.
    store.record_attack(p1_miss);
    store.record_result(p1_miss, AttackResult::Miss);
    store.swap_roles();
    store.round += 1;

    // Hit 2: second cell of Destroyer → it should now be sunk.
    play_scripted_round(&mut store, destroyer_cells[1], AttackResult::Hit, &sp1);
    assert_eq!(
        store.sunk_ships[1],
        vec![4],
        "Destroyer (index 4) should be sunk after both cells hit"
    );
}

/// Verify that attacks on both sides are correctly tracked and 16 hits
/// is not enough to end the game.
#[test]
fn game_does_not_end_at_16_hits() {
    let mut store = GameStore::new();

    let sp0 = ships_p0();
    let sp1 = ships_p1();

    store.set_ships(0, sp0);
    store.set_ships(1, sp1);
    store.set_commitment(0, [1u8; 32]);
    store.set_commitment(1, [2u8; 32]);
    store.set_blinding(0, [10u8; 32]);
    store.set_blinding(1, [20u8; 32]);
    store.round = 1;

    let p0_attacks = all_cells(&sp1);
    let p1_misses: Vec<(u8, u8)> = vec![
        (1, 0), (1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6),
        (3, 0), (3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6),
        (5, 0), (5, 1),
    ];

    // Play 16 hits (not enough to win).
    for i in 0..16 {
        let w = play_scripted_round(&mut store, p0_attacks[i], AttackResult::Hit, &sp1);
        assert!(w.is_none(), "game should not end at {} hits", i + 1);
        let w = play_scripted_round(&mut store, p1_misses[i], AttackResult::Miss, &sp0);
        assert!(w.is_none());
    }

    assert_eq!(store.hit_count_for_defender(1), 16);
    assert!(store.is_game_over().is_none());

    // 17th hit ends it.
    let w = play_scripted_round(&mut store, p0_attacks[16], AttackResult::Hit, &sp1);
    assert_eq!(w, Some(0));
}

/// Both players trade hits.  Player 1 reaches 17 hits first and wins.
#[test]
fn player_1_can_also_win() {
    let mut store = GameStore::new();

    let sp0 = ships_p0();
    let sp1 = ships_p1();

    store.set_ships(0, sp0);
    store.set_ships(1, sp1);
    store.set_commitment(0, [1u8; 32]);
    store.set_commitment(1, [2u8; 32]);
    store.set_blinding(0, [10u8; 32]);
    store.set_blinding(1, [20u8; 32]);
    store.round = 1;

    let p0_cells = all_cells(&sp0);

    // P0 always misses — coords that are:
    //  - NOT P1 ship cells (to be misses on P1's board)
    //  - NOT P0 ship cells (so they don't collide with P1's attacks in the shared attack_log)
    let p0_misses: Vec<(u8, u8)> = vec![
        (9, 0), (9, 1), (9, 2), (9, 3), (9, 4), (9, 5), (9, 6), (9, 7), (9, 8), (9, 9),  // row 9
        (7, 0), (7, 1), (7, 2), (7, 3), (7, 4), (7, 5), (7, 6),                          // row 7
    ];

    let mut winner: Option<usize> = None;

    for i in 0..17 {
        // P0 misses
        let r = expected_result(p0_misses[i], &sp1);
        assert_eq!(r, AttackResult::Miss);
        winner = play_scripted_round(&mut store, p0_misses[i], r, &sp1);
        if winner.is_some() {
            break;
        }

        // P1 hits P0
        let r = expected_result(p0_cells[i], &sp0);
        assert_eq!(r, AttackResult::Hit);
        winner = play_scripted_round(&mut store, p0_cells[i], r, &sp0);
        if winner.is_some() {
            break;
        }
    }

    assert_eq!(winner, Some(1), "Player 1 should win");
    assert_eq!(store.hit_count_for_defender(0), 17);
}
