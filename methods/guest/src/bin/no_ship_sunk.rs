#![no_main]

risc0_zkvm::guest::entry!(main);

use battleship_core::{canonical_preimage, NoShipSunkInput, NoShipSunkOutput};
use risc0_zkvm::guest::env;
use risc0_zkvm::sha::Impl as Sha2Impl;
use risc0_zkvm::sha::Sha256;

fn main() {
    // -----------------------------------------------------------------------
    // 1. Read the private witness.
    // -----------------------------------------------------------------------
    let input: NoShipSunkInput = env::read();

    let ships = &input.ships;
    let blinding = &input.blinding;
    let surviving_cell_indices = &input.surviving_cell_indices;
    let hit_log = &input.hit_log;
    let transcript_length = hit_log.len() as u32;

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
    // 3. For each ship, prove that at least one cell has NOT been hit.
    //
    //    The defender provides one "surviving cell index" per ship —
    //    an index into that ship's own cells (0 .. ship.len()-1).
    //    The circuit verifies:
    //      a) The index is within bounds for that ship.
    //      b) The coordinate of that cell does NOT appear in the hit_log.
    //
    //    If every ship has a proven surviving cell, then no ship has
    //    been fully destroyed.
    // -----------------------------------------------------------------------
    for (ship_i, ship) in ships.iter().enumerate() {
        let surviving_idx = surviving_cell_indices[ship_i] as usize;
        let ship_len = ship.ship_type.len() as usize;

        // (a) Surviving cell index must be within this ship's length.
        assert!(
            surviving_idx < ship_len,
            "surviving_cell_indices[{}] = {} is out of bounds for ship of length {}",
            ship_i,
            surviving_idx,
            ship_len
        );

        // (b) Derive the surviving cell coordinate and check it is not in hit_log.
        let ship_cells: Vec<(u8, u8)> = ship.cells().collect();
        let surviving_coord = ship_cells[surviving_idx];

        let was_hit = hit_log.iter().any(|&coord| coord == surviving_coord);
        assert!(
            !was_hit,
            "ship {} claimed surviving cell ({},{}) is in the hit log",
            ship_i,
            surviving_coord.0,
            surviving_coord.1
        );
    }

    // -----------------------------------------------------------------------
    // 4. Commit the public output to the journal.
    //
    //    The `transcript_length` timestamps the proof — prevents replay.
    //    The verifier checks that this matches the current transcript length.
    // -----------------------------------------------------------------------
    env::commit(&NoShipSunkOutput {
        commitment,
        transcript_length,
    });
}
