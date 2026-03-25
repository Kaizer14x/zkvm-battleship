# ZK Battleship — Rounds 1+ Implementation Plan

> **Goal**: Take the game from "two committed boards" to a fully playable CLI game with ZK-proven hit/miss, mandatory sinking declarations, and winner detection.
>
> **Base documents**: [battleship_game_flow.md](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/plans/battleship_game_flow.md), [round0.md](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/plans/round0.md), [plan.md](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/plans/plan.md)

---

## Current State

Round 0 is **complete and tested**:
- [core/lib.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/core/src/lib.rs) — [Ship](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/core/src/lib.rs#76-84), `ShipType`, `Orientation`, `Direction`, [Player](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/core/src/lib.rs#147-151), `BoardCommitInput/Output`, [normalize()](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/core/src/lib.rs#104-122), [cells()](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/core/src/lib.rs#90-101)
- [validate_board.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/guest/src/bin/validate_board.rs) — guest proof: validates board, computes SHA-256 commitment
- [logic.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/logic.rs) — [round_zero()](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/logic.rs#16-125) orchestrator + host-side pre-validation
- [storage.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/storage.rs) — [GameStore](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/storage.rs#14-19) with commitments, players, round counter
- [display.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/display.rs) — ship placement prompts + ASCII board
- [round0.rs tests](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/tests/round0.rs) — 5 integration tests (valid board, determinism, blinding, overlap, out-of-bounds)

---

## User Review Required

> [!IMPORTANT]
> **Delivery strategy**: Vertical slices — each phase wires a proof end-to-end (core types → guest → host → test) rather than building all proofs in isolation. This means Phase 4 produces a runnable hit/miss game before Phase 5 adds sinking.

> [!IMPORTANT]
> **Trust model**: The host is trusted for turn/order enforcement in this CLI phase. On-chain trustless enforcement is deferred.

> [!WARNING]
> **Scope exclusions**: Networking/multiplayer, persistence/database, on-chain migration, and trustless turn enforcement inside circuits are **not** in scope for this plan.

> [!IMPORTANT]
> **3 guest programs to add**: `hit_miss.rs`, `ship_sunk.rs`, `no_ship_sunk.rs`. Each recomputes the SHA-256 commitment to chain-of-trust-check against the stored `C`. The existing [validate_board.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/guest/src/bin/validate_board.rs) is unchanged.

---

## Proposed Changes

### Phase 0 — Data Contract Freeze

Lock the public/private data contracts for the three new proof families before writing any code. This prevents downstream drift.

| Proof | Private Inputs (witness) | Journal (public output) |
|-------|--------------------------|------------------------|
| **Hit/Miss** | `ships[5]`, `blinding[32]`, `attack_coord (u8,u8)` | `commitment C`, `attack_coord`, `result: HIT/MISS`, `round_number` |
| **Ship Sunk** | `ships[5]`, `blinding[32]`, `sunk_ship_index (u8)`, `hit_log: Vec<(u8,u8)>`, `hit_indices[len]` | `commitment C`, `ship_index`, `transcript_length` |
| **No Ship Sunk** | `ships[5]`, `blinding[32]`, `surviving_cell_indices[5] (u8)`, `hit_log: Vec<(u8,u8)>` | `commitment C`, `transcript_length` |

> This table is the canonical reference. All structs, guest programs, and host checks must match.

---

### Phase 1 — Core Protocol Types

#### [MODIFY] [lib.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/core/src/lib.rs)

Add the following new types to the shared `core` crate:

```rust
// --- Attack result ---
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackResult {
    Hit,
    Miss,
}

// --- Public transcript entry ---
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TranscriptEntry {
    pub coord: (u8, u8),
    pub result: AttackResult,
}

// --- Hit/Miss proof I/O ---
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HitMissInput {
    pub ships: [Ship; 5],
    pub blinding: [u8; 32],
    pub attack_coord: (u8, u8),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HitMissOutput {
    pub commitment: [u8; 32],
    pub attack_coord: (u8, u8),
    pub result: AttackResult,
    pub round_number: u32,
}

// --- Ship Sunk proof I/O ---
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShipSunkInput {
    pub ships: [Ship; 5],
    pub blinding: [u8; 32],
    pub sunk_ship_index: u8,       // 0..4
    pub hit_log: Vec<(u8, u8)>,    // derived from public transcript
    pub hit_indices: Vec<u8>,      // index into hit_log for each cell
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShipSunkOutput {
    pub commitment: [u8; 32],
    pub ship_index: u8,
    pub transcript_length: u32,
}

// --- No Ship Sunk proof I/O ---
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NoShipSunkInput {
    pub ships: [Ship; 5],
    pub blinding: [u8; 32],
    pub surviving_cell_indices: [u8; 5], // one per ship
    pub hit_log: Vec<(u8, u8)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NoShipSunkOutput {
    pub commitment: [u8; 32],
    pub transcript_length: u32,
}
```

Also add a shared helper to recompute the commitment (extracted from [validate_board.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/guest/src/bin/validate_board.rs)):

```rust
/// Recompute the SHA-256 board commitment from ships + blinding.
/// Used by guest programs to chain-of-trust-check against stored C.
/// NOTE: This function is only callable inside the guest (uses risc0 SHA).
/// For host-side, we only compare against the stored commitment.
pub fn canonical_preimage(ships: &[Ship; 5], blinding: &[u8; 32]) -> Vec<u8> {
    let mut preimage: Vec<u8> = Vec::with_capacity(32 + 5 * 3);
    preimage.extend_from_slice(blinding);
    for ship in ships.iter() {
        preimage.push(ship.row);
        preimage.push(ship.col);
        preimage.push(match ship.orientation {
            Orientation::Horizontal => 0u8,
            Orientation::Vertical => 1u8,
        });
    }
    preimage
}
```

---

### Phase 2 — Host State Model & Enforcement Shell

#### [MODIFY] [storage.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/storage.rs)

Extend [GameStore](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/storage.rs#14-19) to hold Round 1+ state:

```rust
pub struct GameStore {
    pub players: [Player; 2],
    pub commitments: [Option<[u8; 32]>; 2],
    pub blindings: [Option<[u8; 32]>; 2],    // NEW: needed to build witnesses for the defender
    pub round: u32,
    // --- Round 1+ state ---
    pub attack_log: Vec<(u8, u8)>,           // NEW: all coords ever attacked
    pub transcript: Vec<TranscriptEntry>,     // NEW: confirmed (coord, result) pairs
    pub sunk_ships: [Vec<u8>; 2],            // NEW: sunk ship indices per player
    pub active_attacker: usize,              // NEW: 0 or 1
}
```

Add helper methods:
- `is_already_attacked(coord) -> bool` — checks attack log
- `hit_log() -> Vec<(u8,u8)>` — filters transcript for HIT entries
- `hit_count_for_defender() -> usize` — hits against the current defender
- `record_attack(coord)` — appends to attack log
- `record_result(coord, result)` — appends to transcript
- `record_sunk(player_id, ship_index)` — records a sunk ship
- `swap_roles()` — swaps attacker/defender
- `is_game_over() -> bool` — returns true if hit log has 17 entries for either player

---

#### [MODIFY] [logic.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/logic.rs)

Add the round orchestrator skeleton:

```rust
/// Run one complete round (Steps 1-4 from game_flow.md):
pub fn play_round(store: &mut GameStore) -> Option<usize> {
    // Step 1: Attack Declaration
    let attack_coord = step_attack_declaration(store);

    // Step 2: Hit/Miss Proof
    let result = step_hit_miss_proof(store, attack_coord);

    // Step 3: Sinking Declaration (mandatory after HIT)
    if result == AttackResult::Hit {
        step_sinking_declaration(store);
    }

    // Step 4: Round Close
    step_round_close(store)
}
```

Each `step_*` function handles:
1. **Attack declaration**: Prompt attacker, validate coord, add to attack log
2. **Hit/Miss proof**: Build `HitMissInput` for defender, call prover, verify receipt, validate journal (commitment matches, coord matches, round matches), append to transcript
3. **Sinking declaration**: Derive hit log from transcript. Prompt defender: "Did a ship sink? (y/n)". If yes → build `ShipSunkInput`, prove, verify. If no → build `NoShipSunkInput`, prove, verify.
4. **Round close**: Check if game over (17 hits). If not, swap roles, increment round.

---

### Phase 3 — Method Embedding Expansion

#### [NEW] [hit_miss.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/guest/src/bin/hit_miss.rs)

New guest program — the hit/miss proof circuit.

**Logic outline:**
1. Read `HitMissInput` from witness
2. Recompute SHA-256 commitment from ships + blinding → assert it matches `C` read from host (or committed in Round 0 — we embed it in the input or use a separate field)
3. Derive all 17 ship cells via [cells()](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/core/src/lib.rs#90-101)
4. Check if `attack_coord` is in any ship's cells → `AttackResult::Hit` or `Miss`
5. Commit `HitMissOutput { commitment, attack_coord, result, round_number }` to journal

> [!NOTE]
> The round number is passed as an additional field in the input and committed to the journal. The host validates it matches the current round after receipt verification.

#### [NEW] [ship_sunk.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/guest/src/bin/ship_sunk.rs)

**Logic outline:**
1. Read `ShipSunkInput`
2. Recompute commitment → assert matches C
3. Assert `sunk_ship_index < 5`
4. Derive cells for the indicated ship
5. For each cell: assert the hit_log entry at the corresponding `hit_indices[i]` exactly equals that cell's coordinate
6. Assert all `hit_indices` are distinct (no double-counting)
7. Commit `ShipSunkOutput { commitment, ship_index, transcript_length }`

#### [NEW] [no_ship_sunk.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/guest/src/bin/no_ship_sunk.rs)

**Logic outline:**
1. Read `NoShipSunkInput`
2. Recompute commitment → assert matches C
3. For each of the 5 ships:
   - Assert `surviving_cell_indices[i] < ship.len()`
   - Derive the cell at that index
   - Assert that cell does NOT appear anywhere in `hit_log`
4. Commit `NoShipSunkOutput { commitment, transcript_length }`

#### [MODIFY] [build.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/build.rs) & [lib.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/methods/src/lib.rs)

No changes needed — `risc0-build` auto-discovers all binaries in `methods/guest/src/bin/`. The `methods/src/lib.rs` uses `include!(concat!(env!("OUT_DIR"), "/methods.rs"))` which auto-exports all image IDs and ELFs. Simply adding new `.rs` files to `bin/` is sufficient.

> [!TIP]
> Verify by checking the generated `methods.rs` output after adding the new guest binaries.

---

### Phase 4 — Vertical Slice A: Hit/Miss End-to-End

#### [MODIFY] [display.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/display.rs)

Add attack-phase display functions:
- `prompt_attack() -> (u8, u8)` — ask attacker for `(row, col)`
- `show_attack_result(coord, result)` — announce HIT or MISS
- `show_shot_board(attack_log, transcript)` — ASCII grid showing attacker's shot history (`.` = unknown, `X` = hit, `O` = miss)

#### [MODIFY] [logic.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/logic.rs)

Implement `step_attack_declaration()` and `step_hit_miss_proof()`:

```rust
fn step_hit_miss_proof(store: &mut GameStore, attack_coord: (u8, u8)) -> AttackResult {
    let defender_id = 1 - store.active_attacker;
    let ships = store.players[defender_id].ships.unwrap();
    let blinding = store.blindings[defender_id].unwrap();

    let input = HitMissInput {
        ships,
        blinding,
        attack_coord,
        // round_number passed via a separate field (see design note)
    };

    let env = ExecutorEnv::builder()
        .write(&input)
        .unwrap()
        .write(&store.round)   // pass round number separately
        .build()
        .unwrap();

    let receipt = default_prover()
        .prove(env, HIT_MISS_ELF)
        .expect("hit/miss prove failed")
        .receipt;

    receipt.verify(HIT_MISS_ID).expect("receipt verification failed");

    let output: HitMissOutput = receipt.journal.decode().unwrap();

    // --- Host-side journal validation ---
    assert_eq!(output.commitment, store.commitments[defender_id].unwrap());
    assert_eq!(output.attack_coord, attack_coord);
    assert_eq!(output.round_number, store.round);

    // Append to transcript
    store.record_result(attack_coord, output.result);
    output.result
}
```

#### [MODIFY] [main.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/main.rs)

After Round 0, add the main game loop:

```rust
// --- Game loop: Rounds 1+ ---
loop {
    show_message(&format!("\n=== Round {} ===", store.round));
    if let Some(winner) = play_round(&mut store) {
        show_message(&format!("\n🎉 Player {} wins!", winner + 1));
        break;
    }
}
```

---

### Phase 5 — Vertical Slice B: Mandatory Sinking Declarations

#### [MODIFY] [logic.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/logic.rs)

Implement `step_sinking_declaration()`:

```rust
fn step_sinking_declaration(store: &mut GameStore) {
    let defender_id = 1 - store.active_attacker;
    let hit_log = store.hit_log_for(defender_id);

    // In CLI mode, the defender knows their own board and can determine
    // if a ship sunk. In a trustless model, the engine would not ask —
    // the defender would simply submit the appropriate proof.
    let sunk = prompt_sinking_declaration();

    if sunk {
        // Path A: prove a specific ship sunk
        let ship_index = prompt_ship_index_to_declare_sunk();
        // Build ShipSunkInput, prove, verify, record
        ...
    } else {
        // Path B: prove no ship has fully sunk
        // Build NoShipSunkInput, prove, verify
        ...
    }
}
```

#### [MODIFY] [display.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/display.rs)

Add sinking prompts:
- `prompt_sinking_declaration() -> bool` — "Did a ship sink? (y/n)"
- `prompt_ship_index_to_declare_sunk() -> u8` — which ship sunk (0-4)
- `show_sinking_announcement(ship_type)` — "🚢 Carrier has been sunk!"
- `show_sunk_ships_summary(sunk_list)`

---

### Phase 6 — Full Round Loop & Winner

#### [MODIFY] [logic.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/logic.rs)

Wire `step_round_close()`: check game-over condition (17 hits against one defender), swap roles, increment round.

#### [MODIFY] [storage.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/storage.rs)

Track per-player hit counts or derive from transcript. Ensure `is_game_over()` checks both directions (since players alternate defending).

#### [MODIFY] [main.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/main.rs)

Complete the game loop with:
- Winner announcement
- Final game state summary (sunk ships, total rounds)
- Prevent any actions after terminal state

---

### Phase 7 — Blinding Factor Storage

#### [MODIFY] [logic.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/logic.rs)

`round_zero()` currently generates a `blinding` factor and discards it after proving. We need to **retain** it so later proofs can re-derive the commitment. Update `round_zero()` to store the blinding:

```rust
store.set_blinding(player_id, blinding);
```

#### [MODIFY] [storage.rs](file:///home/kaizer/Projects/zk-battleship/zkvm-battleship/host/src/storage.rs)

Add `blindings: [Option<[u8; 32]>; 2]` to `GameStore` and a `set_blinding()` method.

> [!CAUTION]
> This is a critical architectural fix. Without the blinding factor, the defender cannot re-derive the commitment in subsequent proofs, breaking the entire chain-of-trust.

---

## Implementation Order (Phases as Steps)

| Step | Phase | What | Key Files | Depends On |
|------|-------|------|-----------|------------|
| 1 | 1 | Core protocol types | `core/src/lib.rs` | — |
| 2 | 7 | Blinding factor storage | `host/src/storage.rs`, `host/src/logic.rs` | — |
| 3 | 2 | Host state model + enforcement shell | `host/src/storage.rs`, `host/src/logic.rs` | 1 |
| 4 | 3 | Guest: `hit_miss.rs` | `methods/guest/src/bin/hit_miss.rs` | 1 |
| 5 | 4 | Wire hit/miss end-to-end | `host/src/logic.rs`, `display.rs`, `main.rs` | 2, 3, 4 |
| 6 | 3 | Guests: `ship_sunk.rs`, `no_ship_sunk.rs` | `methods/guest/src/bin/` | 1 |
| 7 | 5 | Wire sinking declarations end-to-end | `host/src/logic.rs`, `display.rs` | 5, 6 |
| 8 | 6 | Full round loop + winner | `host/src/main.rs`, `logic.rs`, `storage.rs` | 7 |
| 9 | — | Tests + stabilization | `host/tests/` | 8 |

---

## Verification Plan

### Automated Tests

All tests run from the workspace root.

**Existing tests (regression check):**
```bash
cargo test -p battleship-core   # core unit tests
cargo test -p host --test round0  # Round 0 integration tests (5 tests)
```

**New tests to add:**

#### [NEW] `host/tests/hit_miss.rs`
Integration tests for the hit/miss guest program:
1. **Valid HIT**: Place ships, attack a known occupied cell → proof succeeds, journal says HIT
2. **Valid MISS**: Attack an empty cell → proof succeeds, journal says MISS
3. **Commitment mismatch**: Provide wrong blinding → prove fails (guest assertion fires)
4. **Round number in journal**: Verify round number in output matches input

```bash
cargo test -p host --test hit_miss
```

#### [NEW] `host/tests/sinking.rs`
Integration tests for sinking proofs:
1. **Ship sunk proof**: All cells of one ship are in hit log → proof succeeds
2. **Ship sunk — non-distinct indices**: Same hit index used twice → proof fails
3. **No ship sunk proof**: Each ship has at least one un-hit cell → proof succeeds
4. **No ship sunk — false survivor**: Claim a cell as surviving when it is in the hit log → proof fails

```bash
cargo test -p host --test sinking
```

#### [NEW] `host/tests/full_game.rs`
End-to-end integration test:
1. **Happy path to winner**: Two known boards, scripted attack sequence that sinks all ships of one player (17 hits). Verify game ends correctly.
2. **Duplicate attack rejected**: Same coordinate attacked twice → host rejects before proof

```bash
cargo test -p host --test full_game
```

### Manual Verification

1. **Full CLI playthrough**: Run `cargo run -p host` and play a complete game with two players on one terminal. Verify:
   - Attack prompts appear correctly
   - HIT/MISS announced after each proof
   - Sinking declarations mandatory after every HIT
   - Sunk ship type announced
   - Game ends at 17 hits with winner announcement
   - Invalid inputs (duplicate attacks, out-of-bounds) rejected gracefully

2. **Proof generation timing**: Note the wall-clock time for each `prove()` call to establish a baseline. This is informational, not pass/fail.
