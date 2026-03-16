use rand::RngCore;
use risc0_zkvm::{default_prover, ExecutorEnv};

use battleship_core::{normalize, BoardCommitInput, BoardCommitOutput, Ship, ShipType};
use methods::VALIDATE_BOARD_ELF;

use crate::display::{prompt_ship_placement, show_board, show_message, show_ship_placed};
use crate::storage::GameStore;

/// -------------------------------------------------------------------------
///! Run Round 0 for one player:
///-------------------------------------------------------------------------



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

            // Build the candidate ship.
            let ship = Ship {
                ship_type: *ship_type,
                row: norm_row,
                col: norm_col,
                orientation,
            };

            // Basic host-side pre-validation to give instant feedback before
            // incurring proving overhead.  
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

    // Show the completed board before proving.
    {
        let mut preview_player = battleship_core::Player::new(player_id);
        preview_player.ships = Some(ships);
        show_board(&preview_player);
    }

    // -----------------------------------------------------------------------
    // Step 3: Assemble the private witness with a freshly generated blinding
    //         factor.
    // -----------------------------------------------------------------------
    let mut blinding = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut blinding);

    let input = BoardCommitInput { ships, blinding };

    // -----------------------------------------------------------------------
    // Step 4: Execute the guest program in the zkVM.
    //
    //   ExecutorEnv carries the private witness into the guest via
    //   env::read().
    // -----------------------------------------------------------------------
    show_message("  Generating ZK proof (this may take a moment)...");

    //? THE ZKVM IS HERE : 

    let env = ExecutorEnv::builder()
        .write(&input)
        .expect("failed to serialise BoardCommitInput")
        .build()
        .expect("failed to build ExecutorEnv");

    let prover = default_prover();
    let receipt = prover
        .prove(env, VALIDATE_BOARD_ELF)
        .expect("prove() failed — invalid board or internal error")
        .receipt;

    // -----------------------------------------------------------------------
    // Step 5: Extract the public commitment from the journal.
    // -----------------------------------------------------------------------
    let output: BoardCommitOutput = receipt
        .journal
        .decode()
        .expect("failed to decode journal as BoardCommitOutput");

    // -----------------------------------------------------------------------
    // Step 6: Verify the receipt against the known IMAGE_ID.
    //
    //? This is another layer of check (Cryptographyical) 
    // -----------------------------------------------------------------------
    receipt
        .verify(methods::VALIDATE_BOARD_ID)
        .expect("receipt verification failed");

    show_message(&format!(
        "  Proof verified. Board commitment: 0x{}",
        hex_encode(&output.commitment)
    ));

    // -----------------------------------------------------------------------
    // Step 7: Persist.
    //? NOT DEVELOPED YET.
    // -----------------------------------------------------------------------
    store.set_ships(player_id, ships);
    store.set_commitment(player_id, output.commitment);
}

// ---------------------------------------------------------------------------
// Host-side pre-validation (UX only — guest re-checks everything as proofs)
// ---------------------------------------------------------------------------

/// Quick host-side sanity checks so the player gets instant feedback instead
/// of waiting for a full proof attempt only to have it fail.
///
/// Checks performed:
/// 1. Ship stays within grid bounds.
/// 2. No overlap with already-placed ships.
fn pre_validate_placement(candidate: &Ship, placed: &[Ship]) -> Result<(), String> {
    let len = candidate.ship_type.len();

    // Boundary check.
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

    // Overlap check.
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

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
