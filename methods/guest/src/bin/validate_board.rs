#![no_main]

risc0_zkvm::guest::entry!(main);

use battleship_core::{BoardCommitInput, BoardCommitOutput, Orientation, TOTAL_SHIP_CELLS};
use risc0_zkvm::guest::env;
use risc0_zkvm::sha::Impl as Sha2Impl;
use risc0_zkvm::sha::Sha256;

fn main() {

    
    let input: BoardCommitInput = env::read();

    let ships = &input.ships;
    let blinding = &input.blinding;

    // -----------------------------------------------------------------------
    // 2. Validate each ship's boundary conditions.
    //
    //    For each ship:
    //      • anchor must be inside the grid: row < 10 && col < 10
    //      • the ship must not extend past the grid edge:
    //          Horizontal: col + len <= 10
    //          Vertical:   row + len <= 10
    //
    //    Orientation validity is guaranteed by the type system: serde
    //    deserialization panics on unknown enum variants, so if we reach here
    //    all orientations are legal 
    // -----------------------------------------------------------------------
    for ship in ships.iter() {
        let len = ship.ship_type.len();

        assert!(ship.row < 10, "ship row anchor out of bounds");
        assert!(ship.col < 10, "ship col anchor out of bounds");

        match ship.orientation {
            Orientation::Horizontal => {
                assert!(
                    ship.col + len <= 10,
                    "horizontal ship extends past right edge"
                );
            }
            Orientation::Vertical => {
                assert!(
                    ship.row + len <= 10,
                    "vertical ship extends past bottom edge"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // 3. No-overlap check + completeness assertion.
    //
    //    Build a 10×10 occupancy grid.  For every cell a ship wants to occupy:
    //      • assert it is not already taken (no overlap)
    //      • mark it as occupied
    //
    //    After all ships: assert the total occupied cell count == 17.
    //    This rules out both overlaps and duplicate/missing ships.
    // -----------------------------------------------------------------------
    let mut grid = [[false; 10]; 10];
    let mut occupied = 0usize;

    for ship in ships.iter() {
        for (r, c) in ship.cells() {
            assert!(
                !grid[r as usize][c as usize],
                "ships overlap at ({}, {})",
                r, c
            );
            grid[r as usize][c as usize] = true;
            occupied += 1;
        }
    }

    assert!(
        occupied == TOTAL_SHIP_CELLS,
        "expected {} occupied cells, found {}",
        TOTAL_SHIP_CELLS,
        occupied
    );

    // -----------------------------------------------------------------------
    // 4. Compute the board commitment.
    //
    //    Canonical byte string: blinding_salt (32 bytes)
    //    followed by each ship encoded as: row (1 byte) || col (1 byte) ||
    //    orientation (1 byte: 0 = H, 1 = V) — in canonical ShipType order.
    //
    // -----------------------------------------------------------------------
    let mut preimage: Vec<u8> = Vec::with_capacity(32 + 5 * 3);

    // blinding salt
    preimage.extend_from_slice(blinding);

    // ship data in a stable, compact encoding
    for ship in ships.iter() {
        preimage.push(ship.row);
        preimage.push(ship.col);
        preimage.push(match ship.orientation {
            Orientation::Horizontal => 0u8,
            Orientation::Vertical => 1u8,
        });
    }

    let digest = Sha2Impl::hash_bytes(&preimage);
    let commitment: [u8; 32] = digest
        .as_bytes()
        .try_into()
        .expect("SHA-256 digest is 32 bytes");

    // -----------------------------------------------------------------------
    // 5. Commit the public output to the journal.
    //
    //    The journal is the only thing the verifier sees.  Everything else
    //    (ships, blinding) stays private (Pattern 2: Private Witness, Public
    //    Journal).
    // -----------------------------------------------------------------------
    env::commit(&BoardCommitOutput { commitment });
}
