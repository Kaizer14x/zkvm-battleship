use rand::RngCore;
use risc0_zkvm::{default_prover, ExecutorEnv};

use battleship_core::{
    normalize, AttackResult, BoardCommitInput, BoardCommitOutput, HitMissInput, HitMissOutput,
    NoShipSunkInput, NoShipSunkOutput, Ship, ShipSunkInput, ShipSunkOutput, ShipType,
};
use methods::{HIT_MISS_ELF, HIT_MISS_ID, NO_SHIP_SUNK_ELF, NO_SHIP_SUNK_ID, SHIP_SUNK_ELF, SHIP_SUNK_ID, VALIDATE_BOARD_ELF};

use crate::display::{
    prompt_attack, prompt_ship_placement,
    show_attack_result, show_board, show_message, show_ship_placed, show_shot_board,
    show_sinking_announcement, show_sunk_summary,
};
use crate::storage::GameStore;

// =============================================================================
// Round 0 — Board placement and commitment
// =============================================================================

pub fn round_zero(player_id: usize, store: &mut GameStore) {
    show_message(&format!(
        "\n========================================\n\
         Player {} — Board Setup (Round 0)\n\
         ========================================",
        player_id + 1
    ));

    // -----------------------------------------------------------------------
    // Step 1 & 2: Prompt for each ship and normalise.
    // -----------------------------------------------------------------------
    let mut ships_vec: Vec<Ship> = Vec::with_capacity(5);

    for ship_type in ShipType::ALL.iter() {
        loop {
            let (raw_row, raw_col, orientation, direction) = prompt_ship_placement(*ship_type);

            let (norm_row, norm_col) =
                normalize(raw_row, raw_col, orientation, direction, ship_type.len());

            let ship = Ship {
                ship_type: *ship_type,
                row: norm_row,
                col: norm_col,
                orientation,
            };

            if let Err(msg) = pre_validate_placement(&ship, &ships_vec) {
                show_message(&format!("  [invalid] {}\n  Try again.", msg));
                continue;
            }
            show_ship_placed(&ship);
            ships_vec.push(ship);
            break;
        }
    }

    let ships: [Ship; 5] = ships_vec.try_into().expect("exactly 5 ships");

    {
        let mut preview_player = battleship_core::Player::new(player_id);
        preview_player.ships = Some(ships);
        show_board(&preview_player);
    }

    // -----------------------------------------------------------------------
    // Step 3: Assemble private witness with a fresh blinding factor.
    // -----------------------------------------------------------------------
    let mut blinding = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut blinding);

    let input = BoardCommitInput { ships, blinding };

    // -----------------------------------------------------------------------
    // Step 4: Prove.
    // -----------------------------------------------------------------------
    show_message("  Generating ZK proof (this may take a moment)...");

    let env = ExecutorEnv::builder()
        .write(&input)
        .expect("failed to serialise BoardCommitInput")
        .build()
        .expect("failed to build ExecutorEnv");

    let receipt = default_prover()
        .prove(env, VALIDATE_BOARD_ELF)
        .expect("prove() failed — invalid board or internal error")
        .receipt;

    // -----------------------------------------------------------------------
    // Step 5: Extract commitment + verify receipt.
    // -----------------------------------------------------------------------
    let output: BoardCommitOutput = receipt
        .journal
        .decode()
        .expect("failed to decode journal as BoardCommitOutput");

    receipt
        .verify(methods::VALIDATE_BOARD_ID)
        .expect("receipt verification failed");

    show_message(&format!(
        "  Proof verified. Board commitment: 0x{}",
        hex_encode(&output.commitment)
    ));

    // -----------------------------------------------------------------------
    // Step 6: Persist ships, commitment, and blinding factor.
    //         The blinding factor MUST be stored so subsequent proofs can
    // -----------------------------------------------------------------------
    store.set_ships(player_id, ships);
    store.set_commitment(player_id, output.commitment);
    store.set_blinding(player_id, blinding);
}

// =============================================================================
// Rounds : Play loop
// =============================================================================


pub fn play_round(store: &mut GameStore) -> Option<usize> {
    let attacker = store.active_attacker;
    let defender = store.active_defender();

    show_message(&format!(
        "\n--- Round {} | Player {} attacks Player {} ---",
        store.round,
        attacker + 1,
        defender + 1
    ));

    show_shot_board(attacker, &store.transcript);

    let attack_coord = step_attack_declaration(store);

    let result = step_hit_miss_proof(store, attack_coord);
    show_attack_result(attack_coord, result);

    if result == AttackResult::Hit {
        step_sinking_declaration(store);
    }
    step_round_close(store)
}

// =============================================================================
// Step 1 — Attack Declaration
// =============================================================================

fn step_attack_declaration(store: &mut GameStore) -> (u8, u8) {
    loop {
        let coord = prompt_attack(store.active_attacker);

        if coord.0 >= 10 || coord.1 >= 10 {
            show_message("  [rejected] Coordinate out of bounds. Must be 0–9.");
            continue;
        }

        if store.is_already_attacked(coord) {
            show_message("  [rejected] That cell has already been attacked. Choose another.");
            continue;
        }

        //? The attacker cannot retract a declared attack.
        store.record_attack(coord);
        return coord;
    }
}

// =============================================================================
// Step 2 — Hit/Miss Proof
// =============================================================================

fn step_hit_miss_proof(store: &mut GameStore, attack_coord: (u8, u8)) -> AttackResult {
    let defender = store.active_defender();

    let ships = store.players[defender]
        .ships
        .expect("defender ships must be set");
    let blinding = store.blindings[defender].expect("defender blinding must be set");
    let stored_commitment = store.commitments[defender].expect("defender commitment must be set");

    let input = HitMissInput {
        ships,
        blinding,
        attack_coord,
        round_number: store.round,
    };

    show_message(&format!(
        "  [Player {}] Generating hit/miss proof...",
        defender + 1
    ));

    let env = ExecutorEnv::builder()
        .write(&input)
        .expect("serialise HitMissInput")
        .build()
        .expect("build ExecutorEnv");

    let receipt = default_prover()
        .prove(env, HIT_MISS_ELF)
        .expect("hit/miss prove() failed — internal error")
        .receipt;

    receipt.verify(HIT_MISS_ID).expect("hit/miss receipt verification failed");

    let output: HitMissOutput = receipt.journal.decode().expect("decode HitMissOutput");

    // --- Host-side journal validation ---
    assert_eq!(
        output.commitment, stored_commitment,
        "hit/miss proof: commitment mismatch"
    );
    assert_eq!(
        output.attack_coord, attack_coord,
        "hit/miss proof: attack_coord mismatch"
    );
    assert_eq!(
        output.round_number, store.round,
        "hit/miss proof: round_number mismatch"
    );

    store.record_result(attack_coord, output.result);

    output.result
}

// =============================================================================
// Step 3 — Sinking Declaration
// =============================================================================

fn step_sinking_declaration(store: &mut GameStore) {
    let defender = store.active_defender();
    let hit_log = store.hit_log_for_defender(defender);
    let transcript_length = hit_log.len() as u32;
    let stored_commitment = store.commitments[defender].expect("defender commitment must be set");
    let ships = store.players[defender]
        .ships
        .expect("defender ships must be set");
    let blinding = store.blindings[defender].expect("defender blinding must be set");

    // The engine automatically determines whether a ship has sunk by
    // checking the hit log against every unsunk ship's cells.
    if let Some(ship_index) = find_newly_sunk_ship(&ships, &hit_log, &store.sunk_ships[defender]) {
        // ---------------------------------------------------------------
        // Path A — A ship has sunk: prove it.
        // ---------------------------------------------------------------
        let sunk_ship = &ships[ship_index as usize];
        let ship_cells: Vec<(u8, u8)> = sunk_ship.cells().collect();

        let hit_indices: Vec<u8> = ship_cells
            .iter()
            .map(|cell| {
                hit_log
                    .iter()
                    .position(|h| h == cell)
                    .expect("sunk ship cell not found in hit log") as u8
            })
            .collect();

        let input = ShipSunkInput {
            ships,
            blinding,
            sunk_ship_index: ship_index,
            hit_log: hit_log.clone(),
            hit_indices,
        };


        show_message("  Generating ship-sunk proof...");

        let env = ExecutorEnv::builder()
            .write(&input)
            .expect("serialise ShipSunkInput")
            .build()
            .expect("build ExecutorEnv");

        let receipt = default_prover()
            .prove(env, SHIP_SUNK_ELF)
            .expect("ship_sunk prove() failed")
            .receipt;

        receipt.verify(SHIP_SUNK_ID).expect("ship_sunk receipt verification failed");

        let output: ShipSunkOutput = receipt.journal.decode().expect("decode ShipSunkOutput");

        // Host validation.
        assert_eq!(output.commitment, stored_commitment, "ship_sunk: commitment mismatch");
        assert_eq!(
            output.transcript_length, transcript_length,
            "ship_sunk: stale transcript length"
        );
        assert_eq!(output.ship_index, ship_index, "ship_sunk: ship_index mismatch");

        store.record_sunk(defender, ship_index);

        let sunk_type = ShipType::ALL[ship_index as usize];
        show_sinking_announcement(defender, sunk_type);
    } else {
        // ---------------------------------------------------------------
        // Path B — No ship has sunk yet: prove it.
        // ---------------------------------------------------------------
        let surviving_cell_indices =
            find_surviving_indices(&ships, &hit_log)
                .expect("engine: no sunk ship detected but a ship has all cells hit");

        let input = NoShipSunkInput {
            ships,
            blinding,
            surviving_cell_indices,
            hit_log: hit_log.clone(),
        };

        show_message("  Generating no-ship-sunk proof...");

        let env = ExecutorEnv::builder()
            .write(&input)
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

        let output: NoShipSunkOutput =
            receipt.journal.decode().expect("decode NoShipSunkOutput");

        // Host validation.
        assert_eq!(output.commitment, stored_commitment, "no_ship_sunk: commitment mismatch");
        assert_eq!(
            output.transcript_length, transcript_length,
            "no_ship_sunk: stale transcript length"
        );
    }
}

// =============================================================================
// Step 4 — Round Close
// =============================================================================

/// Returns Some(winner_id) if the game has ended, None to continue.
fn step_round_close(store: &mut GameStore) -> Option<usize> {
    if let Some(winner) = store.is_game_over() {
        return Some(winner);
    }

    // Show the current sunk-ship tally before swapping.
    show_sunk_summary(store);

    store.swap_roles();
    store.round += 1;
    None
}

// =============================================================================
// Private helpers
// =============================================================================

/// Host-side pre-validation for ship placement (UX — guest re-checks as proof).
fn pre_validate_placement(candidate: &Ship, placed: &[Ship]) -> Result<(), String> {
    let len = candidate.ship_type.len();

    match candidate.orientation {
        battleship_core::Orientation::Horizontal => {
            if candidate.col as u16 + len as u16 > 10 {
                return Err(format!(
                    "{} extends past the right edge (col {} + len {} > 10)",
                    candidate.ship_type.name(),
                    candidate.col,
                    len
                ));
            }
        }
        battleship_core::Orientation::Vertical => {
            if candidate.row as u16 + len as u16 > 10 {
                return Err(format!(
                    "{} extends past the bottom edge (row {} + len {} > 10)",
                    candidate.ship_type.name(),
                    candidate.row,
                    len
                ));
            }
        }
    }

    let candidate_cells: std::collections::HashSet<(u8, u8)> = candidate.cells().collect();
    for existing in placed {
        for cell in existing.cells() {
            if candidate_cells.contains(&cell) {
                return Err(format!(
                    "{} overlaps with {} at ({}, {})",
                    candidate.ship_type.name(),
                    existing.ship_type.name(),
                    cell.0,
                    cell.1
                ));
            }
        }
    }
    Ok(())
}

/// Returns the index of the first ship that has all its cells in the hit log
/// and has not yet been recorded as sunk, or None if no ship has sunk.
fn find_newly_sunk_ship(ships: &[Ship; 5], hit_log: &[(u8, u8)], already_sunk: &[u8]) -> Option<u8> {
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

/// For Path B (no ship sunk): finds one surviving (un-hit) cell index per ship.
/// Returns None if any ship has all its cells in the hit log (i.e., it IS sunk).
fn find_surviving_indices(
    ships: &[Ship; 5],
    hit_log: &[(u8, u8)],
) -> Option<[u8; 5]> {
    let mut out = [0u8; 5];
    for (i, ship) in ships.iter().enumerate() {
        let cells: Vec<(u8, u8)> = ship.cells().collect();
        let survivor_idx = cells.iter().position(|c| !hit_log.contains(c))?;
        out[i] = survivor_idx as u8;
    }
    Some(out)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
