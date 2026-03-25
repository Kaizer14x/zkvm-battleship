# ZK Battleship — Round 0 Implementation Plan

> Scope: Project scaffolding, shared types, board validity proof, and minimal game loop for Round 0 (ship placement + validation).

---

## 0. Current State

- **Project**: Empty `cargo new` scaffold (`src/main.rs` = "Hello, world!")
- **Toolchain**: `cargo-risczero 3.0.5` installed, `risc0` rustup toolchain available
- **No risc0 integration exists yet** — no workspace, no guest programs, no dependencies

---

## 1. Key Technical Decision: SHA-256 vs Poseidon

The original planning doc recommends Poseidon over SHA-256, reasoning that Poseidon costs ~200-300 constraints vs ~30,000 for SHA-256. **This reasoning applies to arithmetic-circuit proof systems (Groth16, PLONK) but NOT to RISC Zero.**

In RISC Zero, SHA-256 has a **hardware-accelerated precompile** — it is a syscall in the zkVM, not computed instruction-by-instruction. Poseidon, on the other hand, would run as regular RISC-V code without acceleration.

| Hash     | Cost in risc0         | Dependency                               |
|----------|-----------------------|------------------------------------------|
| SHA-256  | **Cheap** (precompile) | Built into `risc0-zkvm`                 |
| Poseidon | **Expensive** (no precompile) | Needs external crate, must compile for riscv32im |

**Decision: Use SHA-256.**

```
C = SHA256(blinding_salt || row0 || col0 || o0 || ... || row4 || col4 || o4)
```

> **Learning note**: This is a key ZK architectural lesson — *the optimal primitive depends on the proof system*. Poseidon is arithmetic-friendly (cheap in R1CS/PLONK circuits), but in a zkVM like risc0, what matters is which operations have hardware precompile acceleration.

**Other decisions:**
- Rust edition: **2021** (risc0 3.0.5 templates use 2021; edition 2024 may have compatibility issues with the riscv32im compilation target)

---

## 2. Project Structure

We restructure the flat crate into a **risc0 workspace** with an added `core` crate:

```
zkvm-battleship/
├── Cargo.toml                      # workspace root
├── rust-toolchain.toml             # pin to stable
├── core/                           # shared library (host + guest both import)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                  # Ship, ShipType, Orientation, Player, BoardCommitment
│
├── host/                           # game engine binary (runs natively)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                 # entry point, game loop orchestrator
│       ├── logic.rs                # Round 0 logic: placement, normalization, proof invocation
│       ├── storage.rs              # in-memory state holder (trivial for now)
│       └── display.rs              # CLI prompts (minimal: println + stdin)
│
├── methods/                        # risc0 methods crate (build infrastructure)
│   ├── Cargo.toml
│   ├── build.rs                    # calls risc0_build::embed_methods()
│   ├── src/
│   │   └── lib.rs                  # include!(methods.rs) — exports ELF + IMAGE_ID
│   └── guest/                      # guest programs (run inside zkVM)
│       ├── Cargo.toml
│       └── src/
│           └── bin/
│               └── validate_board.rs   # Round 0 proof
│
└── plans/                          # planning documents (unchanged)
```

### Why 4 crates?

This mirrors the risc0 convention (host + methods + methods/guest) plus a `core` crate for types shared between host and guest. The `core` crate exists because the same `Ship`, `ShipType`, `Orientation` types must be available on both sides — define once, use everywhere. This is the **"shared kernel" pattern**.

---

## 3. Crate Dependency Graph

```
host ──depends on──> core
host ──depends on──> methods  (for ELF + IMAGE_ID constants)
host ──depends on──> risc0-zkvm  (prover API)

methods ──build-dep──> risc0-build
methods/guest ──depends on──> core
methods/guest ──depends on──> risc0-zkvm  (guest API: env::read, env::commit)
```

Dependencies per crate:

| Crate          | Dependencies                                            |
|----------------|---------------------------------------------------------|
| `core`         | `serde` (with derive)                                   |
| `host`         | `core`, `methods`, `risc0-zkvm ^3.0.5`, `serde`, `tracing-subscriber` |
| `methods`      | `risc0-build ^3.0.5` (build-dep only)                   |
| `methods/guest`| `core`, `risc0-zkvm ^3.0.5` (no default features + std) |

---

## 4. Implementation Phases

### Phase 1: Workspace Scaffolding
- Replace root `Cargo.toml` with a workspace definition
- Create the 4 crate directories with their `Cargo.toml` files
- Add `rust-toolchain.toml` (pin to stable)
- Add `methods/build.rs` and `methods/src/lib.rs` (risc0 boilerplate)
- Verify it compiles with an empty guest (`fn main() {}`)

### Phase 2: Core Types (`core/src/lib.rs`)
- Define `Orientation`, `ShipType`, `Ship`, `Player`
- All types derive `serde::Serialize, serde::Deserialize, Clone, Debug`
- Implement `ShipType::len()`, `Ship::cells()`, normalization function
- Define `BoardCommitInput` — the struct sent as private witness to the guest
- Define `BoardCommitOutput` — the struct committed to the journal (the public commitment)

### Phase 3: Guest Program (`methods/guest/src/bin/validate_board.rs`)
- Read private input: `BoardCommitInput { ships: [Ship; 5], blinding: [u8; 32] }`
- Run the 3 validation checks (orientation boolean, boundary, no-overlap + cell count == 17)
- Compute `SHA256(blinding || serialized_ships)` → commitment `C`
- Commit `C` to the journal via `env::commit()`

### Phase 4: Host Modules
- **`display.rs`**: Prompt player for ship placement (ship type, row, col, orientation, direction). Parse stdin. Minimal — just `println!` + `stdin().read_line()`.
- **`storage.rs`**: `GameStore` struct holding `Player` state for both players plus their commitments. Just fields in a struct, no persistence.
- **`logic.rs`**: `round_zero(player_id, store)` function that:
  1. Calls display to prompt for 5 ship placements
  2. Normalizes each placement
  3. Constructs `BoardCommitInput`
  4. Builds `ExecutorEnv`, writes the input
  5. Calls `default_prover().prove(env, VALIDATE_BOARD_ELF)`
  6. Extracts commitment from receipt journal
  7. Verifies receipt
  8. Stores commitment in `GameStore`

### Phase 5: Main Game Loop (`host/src/main.rs`)
- Initialize tracing
- Print welcome message
- Run `round_zero` for Player 1
- Run `round_zero` for Player 2
- Print both commitments
- Exit (Round 1+ is future work)

### Phase 6: Test & Verify
- Run the full flow end-to-end
- Test with a valid board → proof succeeds
- Test with an invalid board (overlapping ships) → proof generation panics (guest assertion fails)

---

## 5. Detailed Design Decisions

### 5.1 The Commitment Scheme (SHA-256)

```rust
// In core:
#[derive(Serialize, Deserialize)]
struct BoardCommitInput {
    ships:    [Ship; 5],
    blinding: [u8; 32],  // random salt, prevents brute-force
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BoardCommitOutput {
    commitment: [u8; 32],  // SHA-256 digest
}
```

The blinding factor is generated by the host using `rand` and kept private. It prevents an adversary from brute-forcing all possible board layouts (there are only ~30 billion valid placements — tractable without blinding).

### 5.2 Normalization

The user inputs `(row, col, axis, direction)`. The engine immediately normalizes to `(row, col, orientation)` where `(row, col)` is always the top-left anchor. Direction is discarded after normalization.

```rust
fn normalize(row: u8, col: u8, axis: Orientation, direction: Direction, len: u8) -> (u8, u8) {
    match (axis, direction) {
        (Horizontal, Left)  => (row, col - len + 1),
        (Horizontal, Right) => (row, col),
        (Vertical,   Up)    => (row - len + 1, col),
        (Vertical,   Down)  => (row, col),
    }
}
```

This means:
- The `Ship` struct never stores direction
- All downstream logic (cells derivation, commitment, guest validation) works with normalized coordinates only
- The normalization function lives in `core` (shared between host and guest)

### 5.3 Display Module (Minimal)

For Round 0, the display module needs:
- `prompt_ship_placement(ship_type: ShipType) -> (u8, u8, Orientation, Direction)` — prompts the user
- `show_board(player: &Player)` — prints a 10x10 ASCII grid with ship positions
- `show_message(msg: &str)` — generic message printer

No colors, no TUI. Just functional text prompts on stdin/stdout.

### 5.4 Storage Module (Minimal)

```rust
struct GameStore {
    players:     [Player; 2],
    commitments: [Option<[u8; 32]>; 2],
    round:       u32,
}
```

Everything lives in memory. No serialization to disk. When the program exits, state is lost. Persistence is a future concern.

### 5.5 Guest Validation Logic

The guest runs these checks in order:

1. **Orientation is valid**: Enforced by Rust's type system (`enum Orientation`). Serde deserialization fails on invalid variants — this is structurally guaranteed, no explicit check needed.

2. **Boundary conditions**: For each ship, verify `row < 10 && col < 10` and that the ship does not extend past the grid edge (`col + len <= 10` if horizontal, `row + len <= 10` if vertical).

3. **No overlap + completeness**: Build a `[[bool; 10]; 10]` occupancy grid. For each ship's derived cells, assert the cell is unoccupied, then mark it. After all ships, assert total occupied cells == 17.

4. **Compute commitment**: Hash the canonical ship data with the blinding salt. Commit to journal.

Every `assert!()` in the guest is a **proof obligation** — if any assertion fails, no proof can be generated. The verifier never sees failed proofs; they simply do not exist.

---

## 6. ZK Patterns to Study in This Project

### Pattern 1: "The Guest is the Truth Machine"
Everything the guest program does is proven. If it runs to completion, the proof certifies that every line of code executed correctly. `assert!()` becomes a *constraint* — not a runtime check, but a mathematical guarantee. Think of the guest as writing a mathematical proof by executing code.

### Pattern 2: "Private Witness, Public Journal"
- `env::read()` → **private input** (the "witness"). Only the prover knows this.
- `env::commit()` → **public output** (the "journal"). Everyone can see this.
- The proof says: *"I know some private data that, when processed by this program, produces this public output — and all assertions passed."*

### Pattern 3: "Commit, Then Prove Against the Commitment"
This is the foundational ZK pattern in this game:
1. Round 0: Hash your board → publish the hash (commitment)
2. Round 1+: Each proof re-hashes the board and asserts `hash == published_commitment`
3. This guarantees you are proving against the *same* board every time

If you changed your board, the hash would change, the assertion would fail, and the proof would not be generated. The commitment is an **anchor**.

### Pattern 4: "Derive, Don't Store"
Cells are computed from `(row, col, orientation)` — never stored. This:
- Minimizes the witness size (less data to send to the guest)
- Makes the representation canonical (no duplicate representations of the same board)
- Reduces what needs to be hashed in the commitment

### Pattern 5: "Structural Impossibility > Explicit Checks"
Diagonal placement is impossible not because we check for it, but because the `Orientation` enum only has two variants and `cells()` only moves along one axis. This is **correctness by construction** — the strongest form of guarantee in both ZK and software engineering.

### Pattern 6: "The Verifier Sees Only the Journal"
Design your system by asking: *"What does the verifier need to believe, and what is the minimum public data to enable that?"* Everything else stays private. In Round 0, the verifier only needs to know that *some valid board exists* behind commitment `C`. They do not need to know the board itself.

---

## 7. Strategic / Architectural Thinking

### Why Three Host Modules?
The logic/storage/display separation is the **Ports & Adapters** (Hexagonal Architecture) pattern:
- **Logic** is the core domain — it knows the rules but not how data is stored or shown
- **Storage** is a port — today it is RAM, tomorrow it could be SQLite or a blockchain
- **Display** is a port — today it is CLI, tomorrow it could be a web UI or a multiplayer protocol

By keeping these separated from day one, you can swap any adapter without touching the game rules. This is especially relevant when you later move to blockchain (storage changes) or multiplayer (display/networking changes).

### Why a Shared `core` Crate?
The `core` crate is the **canonical source of truth** for types. If `Ship` is defined separately in both host and guest, they could drift apart. A shared crate makes divergence *structurally impossible*. This is critical in ZK: the host and guest must agree on data formats, or serialization will silently produce wrong inputs.

### The "Build in Layers" Strategy
Round 0 is the foundation. Get it right, and every subsequent proof (hit/miss, sinking) follows the same pattern:
1. Host constructs input from private state + public transcript
2. Host sends input to guest via `env::write()`
3. Guest validates, computes, commits results to journal
4. Host extracts public outputs from receipt
5. Host verifies receipt

This is the **same skeleton** for every proof in the game. Once you internalize this cycle, adding new proof types is mechanical.

---

## 8. Implementation Order

| Step | What                              | Why this order                                       |
|------|-----------------------------------|------------------------------------------------------|
| 1    | Workspace + crate scaffolds       | Everything depends on structure                      |
| 2    | Core types + `cells()` + normalize| Guest and host both need these before anything else  |
| 3    | Guest `validate_board.rs`         | The proof program is the hard part — do it early     |
| 4    | `display.rs` (placement prompts)  | Need user input to feed the proof                    |
| 5    | `storage.rs` (trivial struct)     | Need somewhere to hold state                         |
| 6    | `logic.rs` (wire it together)     | Orchestrates display → core → prover → storage       |
| 7    | `main.rs` (game loop)             | Calls logic for P1 then P2                           |
| 8    | End-to-end test                   | Prove a valid board, verify the receipt              |
