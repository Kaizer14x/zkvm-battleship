# ZK Battleship — Implementation Decisions

> Scope: Data structures, game engine architecture, and the board validity proof (Round 0).

---

## 1. Data Structures

### 1.1 Orientation

```rust
enum Orientation {
    Horizontal,
    Vertical,
}
```

- Only two valid values, enforced by the type system.
- No diagonal is representable — diagonal placement is structurally impossible by construction.
- In the guest, orientation drives cell derivation via the formula below. No separate diagonal check is needed.

---

### 1.2 ShipType

```rust
enum ShipType {
    Carrier,      // length 5
    Battleship,   // length 4
    Cruiser,      // length 3
    Submarine,    // length 3
    Destroyer,    // length 2
}

impl ShipType {
    fn len(&self) -> u8 {
        match self {
            Carrier    => 5,
            Battleship => 4,
            Cruiser    => 3,
            Submarine  => 3,
            Destroyer  => 2,
        }
    }
}
```

- Lengths are fixed constants, not stored per instance.
- Total occupied cells across all 5 ships: **17**.

---

### 1.3 Ship

```rust
struct Ship {
    ship_type:   ShipType,
    row:         u8,   // normalized: topmost cell (vertical) or leftmost cell (horizontal)
    col:         u8,
    orientation: Orientation,
}

impl Ship {
    fn cells(&self) -> Vec<(u8, u8)> {
        (0..self.ship_type.len()).map(|j| match self.orientation {
            Orientation::Horizontal => (self.row, self.col + j),
            Orientation::Vertical   => (self.row + j, self.col),
        }).collect()
    }
}
```

**Decisions made:**
- Internal representation is `(row, col, orientation)`. Cells are **not stored** — they are derived on demand by `cells()`.
- The starting point is always **normalized** to the topmost cell (vertical) or leftmost cell (horizontal). Direction is a user-facing input only and is discarded after normalization.
- The `cells()` method is used identically in both host (for display) and guest (for proving). Defined once in the shared core crate.

**What was rejected and why:**
- `(type, [cells])` representation was rejected. Storing cells directly does not eliminate verification work — you would still need to prove the stored cells form a valid non-diagonal line of correct length. The `(row, col, orientation)` representation with derived cells is simpler, smaller, and cleaner to prove.
- Storing direction as a permanent field was rejected. Normalizing at input time removes direction from all downstream logic.

---

### 1.4 Player

```rust
struct Player {
    ships: [Ship; 5],
    // Fixed order: [Carrier, Battleship, Cruiser, Submarine, Destroyer]
}
```

- Exactly 5 ships, fixed order by type.
- Fixed order means both host and guest always agree on which index is which type — no ambiguity.

---

### 1.5 GameState (host-side)

```rust
struct GameState {
    player1:    Player,
    player2:    Player,
    transcript: Vec<(u8, u8)>,   // public shot history, append-only
    round:      u32,
}
```

- The transcript is **public and append-only**. Every shot ever fired lives here.
- Round 0 is the placement and validation phase. Rounds 1+ are play rounds.

---

## 2. User Input & Normalization

The user provides:

```
(ship_type, starting_row, starting_col, axis, direction)
```

The game engine normalizes immediately:

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

Direction is **never stored** beyond this point.

---

## 3. Project Structure

```
battleship/
  ├── core/                          ← shared library (imported by both host and guest)
  │     └── src/lib.rs
  │           Ship, ShipType, Orientation, Player, GameState
  │           poseidon_commit() helper
  │           cells() derivation logic
  │
  ├── host/                          ← game engine (normal Rust binary)
  │     └── src/main.rs
  │           manages rounds
  │           owns private witness
  │           calls prover
  │           reads journal (public outputs)
  │           reads stdout (private state updates from guest)
  │
  └── methods/guest/                 ← proof programs (run inside zkVM)
        └── src/bin/
              ├── validate_board.rs  ← Round 0, one-time
              ├── hit_miss.rs        ← every round
              ├── ship_sunk.rs       ← when a ship sinks
              └── no_ship_sunk.rs    ← when nothing has sunk (after every hit)
```

---

## 4. Host / Guest I/O Model

Each guest program is **stateless**. It remembers nothing between invocations. The host reconstructs full input from private storage and the public transcript before every proof.

```
Channel     Direction           Content in Battleship
─────────────────────────────────────────────────────────────────
stdin       host → guest        ships, blinding, commitment C,
                                attack coordinate, transcript T,
                                hit indices (for sinking proof)

stdout      guest → host        private: updated internal hit map,
                                remaining fleet state
                                (never made public)

journal     guest → world       public: commitment C, hit/miss result,
                                ship type when sunk, transcript length
```

---

## 5. The Poseidon Commitment

```
C = Poseidon(r, row₀, col₀, o₀, row₁, col₁, o₁, row₂, col₂, o₂,
                row₃, col₃, o₃, row₄, col₄, o₄)
```

- `r` is the private blinding factor. Prevents brute-forcing the commitment.
- `C` is computed **inside the guest**, committed to the journal in Round 0.
- Every subsequent guest program recomputes `C` from the witness and asserts equality with the public `C`. This is the chain that ties all proofs to the same board.
- Poseidon is used instead of SHA256 because it is designed for ZK: it operates natively over the same field as the proof system and costs ~200-300 constraints vs ~30,000 for SHA256.

---

## 6. Board Validity Proof — Round 0

**Type:** One-time proof, generated once by each player at game start.

### I/O

```
stdin (private):
  ships:    [(row, col, orientation); 5]
  blinding: [u8; 32]

journal (public):
  commitment C
```

### What the guest proves

**Check 1 — Orientations are boolean**
```rust
for each ship:
    assert!(o == 0 || o == 1);
```

**Check 2 — Boundary conditions**
```rust
for each ship (row, col, o) with length len:
    assert!(row < 10 && col < 10);
    if horizontal: assert!(col + len <= 10);
    if vertical:   assert!(row + len <= 10);
```

**Check 3 — No overlapping ships**
```rust
let mut grid = [[false; 10]; 10];
for each ship:
    for each cell (r, c) derived from ship:
        assert!(!grid[r][c]);   // cell not already occupied
        grid[r][c] = true;
```
Additionally asserts total occupied cells == 17.

**Check 4 — Commitment**
```rust
let C = poseidon_hash(blinding, ships);
env::commit(&C);
```

### Why diagonal is not a separate check

Cells are derived by the formula:
```
cell_j = (row + j,   col    )   if vertical
cell_j = (row,       col + j)   if horizontal
```
Only one coordinate ever changes. Diagonal placement cannot be produced by this formula. The check is structural, not explicit.

### Why no-overlap also proves all ships are present

The grid accumulation check builds a 100-cell boolean map. At the end, the total count of `true` cells is asserted to equal 17. If any ship is missing or has the wrong length, the count fails. Overlap and completeness are proven by the same single pass.

---

## 7. Key Patterns Referenced in This Proof

| Pattern | Used for |
|---|---|
| Boolean constraint `x*(1-x)=0` | Orientation validation |
| Range proof via bit decomposition | Boundary checking |
| ZK multiplexer `o*a + (1-o)*b` | Orientation-driven cell derivation |
| Derive, don't store | Cells computed from `(row,col,o)`, never stored |
| Poseidon commitment | Anchoring all proofs to the same board |
| Assert as proof obligation | Every `assert!()` in guest is a constraint |
