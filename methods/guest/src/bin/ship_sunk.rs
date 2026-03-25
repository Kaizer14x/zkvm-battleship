#![no_main]

risc0_zkvm::guest::entry!(main);

use battleship_core::{canonical_preimage, ShipSunkInput, ShipSunkOutput, ShipType};
use risc0_zkvm::guest::env;
use risc0_zkvm::sha::Impl as Sha2Impl;
use risc0_zkvm::sha::Sha256;

fn main() {
    // -----------------------------------------------------------------------
    // 1. Read the private witness.
    // -----------------------------------------------------------------------
    let input: ShipSunkInput = env::read();

    let ships = &input.ships;
    let blinding = &input.blinding;
    let sunk_ship_index = input.sunk_ship_index as usize;
    let hit_log = &input.hit_log;
    let hit_indices = &input.hit_indices;
    let transcript_length = input.hit_log.len() as u32; // length of hit_log, acts as timestamp

    // -----------------------------------------------------------------------
    // 2. Recompute the SHA-256 commitment (chain-of-trust check).
    // -----------------------------------------------------------------------
    let preimage = canonical_preimage(ships, blinding);
    let digest = Sha2Impl::hash_bytes(&preimage);
    let commitment: [u8; 32] = digest
        .as_bytes()
        .try_into()
        .expect("SHA-256 digest is 32 bytes");

    // -----------------------------------------------------------------------
    // 3. Validate the declared ship index.
    // -----------------------------------------------------------------------
    assert!(sunk_ship_index < 5, "sunk_ship_index must be 0..4");

    let sunk_ship = &ships[sunk_ship_index];
    let ship_len = sunk_ship.ship_type.len() as usize;

    // -----------------------------------------------------------------------
    // 4. Validate hit_indices length matches ship length.
    // -----------------------------------------------------------------------
    assert!(
        hit_indices.len() == ship_len,
        "hit_indices length must equal sunk ship length"
    );

    // -----------------------------------------------------------------------
    // 5. For each cell of the sunk ship, verify that the hit_log entry at the
    //    provided index contains the exact coordinate of that cell.
    //
    //    This proves every cell of the ship has a corresponding entry in the
    //    public hit log.
    // -----------------------------------------------------------------------
    let ship_cells: Vec<(u8, u8)> = sunk_ship.cells().collect();

    for (cell_i, &log_i) in hit_indices.iter().enumerate() {
        let log_i = log_i as usize;
        assert!(
            log_i < hit_log.len(),
            "hit_index {} out of bounds for hit_log of length {}",
            log_i,
            hit_log.len()
        );
        let logged_coord = hit_log[log_i];
        let expected_coord = ship_cells[cell_i];
        assert!(
            logged_coord == expected_coord,
            "hit_log[{}] = ({},{}) does not match ship cell ({},{})",
            log_i,
            logged_coord.0,
            logged_coord.1,
            expected_coord.0,
            expected_coord.1
        );
    }

    // -----------------------------------------------------------------------
    // 6. Assert all hit_indices are distinct.
    //    Prevents counting the same shot twice for different cells.
    //
    //    For small ship lengths (2–5), an O(n²) check is fine.
    // -----------------------------------------------------------------------
    for i in 0..hit_indices.len() {
        for j in (i + 1)..hit_indices.len() {
            assert!(
                hit_indices[i] != hit_indices[j],
                "hit_indices[{}] and hit_indices[{}] are both {} — duplicate not allowed",
                i,
                j,
                hit_indices[i]
            );
        }
    }

    // -----------------------------------------------------------------------
    // 7. Commit the public output to the journal.
    //
    //    The `transcript_length` timestamps the proof — prevents a stale
    //    sinking proof from being replayed in a later round.
    // -----------------------------------------------------------------------
    env::commit(&ShipSunkOutput {
        commitment,
        ship_index: sunk_ship_index as u8,
        transcript_length,
    });
}
