#![no_main]

risc0_zkvm::guest::entry!(main);

use battleship_core::{
    canonical_preimage, AttackResult, HitMissInput, HitMissOutput,
};
use risc0_zkvm::guest::env;
use risc0_zkvm::sha::Impl as Sha2Impl;
use risc0_zkvm::sha::Sha256;

fn main() {
    // -----------------------------------------------------------------------
    // 1. Read the private witness.
    // -----------------------------------------------------------------------
    let input: HitMissInput = env::read();

    let ships = &input.ships;
    let blinding = &input.blinding;
    let attack_coord = input.attack_coord;
    let round_number = input.round_number;

    // -----------------------------------------------------------------------
    // 2. Recompute the SHA-256 commitment from the witness and produce the
    //    commitment hash.
    // -----------------------------------------------------------------------
    let preimage = canonical_preimage(ships, blinding);
    let digest = Sha2Impl::hash_bytes(&preimage);
    let commitment: [u8; 32] = digest
        .as_bytes()
        .try_into()
        .expect("SHA-256 digest is 32 bytes");

    // -----------------------------------------------------------------------
    // 3. Determine hit or miss.
    //
    //    For each ship, derive all occupied cells.  If the attack coordinate
    //    matches any cell, it is a HIT.
    // -----------------------------------------------------------------------
    let mut is_hit = false;

    for ship in ships.iter() {
        for (r, c) in ship.cells() {
            if (r, c) == attack_coord {
                is_hit = true;
            }
        }
    }

    let result = if is_hit {
        AttackResult::Hit
    } else {
        AttackResult::Miss
    };

    // -----------------------------------------------------------------------
    // 4. Commit the public output to the journal.
    //
    //    The journal contains:
    //      - commitment C  (chain-of-trust anchor)
    //      - attack_coord  (which cell was attacked)
    //      - result        (HIT or MISS)
    //      - round_number  (prevents replay across rounds)
    // -----------------------------------------------------------------------
    env::commit(&HitMissOutput {
        commitment,
        attack_coord,
        result,
        round_number,
    });
}
