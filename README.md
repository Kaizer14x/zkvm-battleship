# zkVM Battleship

A two-player Battleship game where every move is backed by zero-knowledge proofs. Built on [RISC Zero](https://risczero.com/) zkVM, the game enforces honest play without revealing private board state — each hit, miss, and sinking announcement is cryptographically proven.

## Quick Start

### Prerequisites

- **Rust 1.92.0** (set automatically via `rust-toolchain.toml`)
- **RISC Zero toolchain** — install via [rzup](https://dev.risczero.com/api/zkvm/install)

### Run the Game

```bash
# Full ZK proof generation (slower, cryptographically verified)
cargo run --release

# Dev mode — skips proof generation for fast iteration
RISC0_DEV_MODE=1 cargo run
```

### Run Tests

```bash
RISC0_DEV_MODE=1 cargo test
```

## How It Works

### Game Rules

Standard Battleship: two players, 10×10 grids, 5 ships each (Carrier 5, Battleship 4, Cruiser 3, Submarine 3, Destroyer 2). Ships are placed horizontally or vertically with no overlaps. Players alternate shots; hits and misses are announced. When all cells of a ship are hit, the defender **must** announce the sinking and prove it. First to sink all 17 ship cells wins.

### ZK Flow

The game uses a **commit-then-prove** pattern. All proofs chain back to the Round 0 board commitment, ensuring the defender cannot alter their board mid-game.

#### Round 0 — Board Commitment

Each player places their 5 ships, then the guest program:
1. Validates placement (bounds, no overlaps, correct ship sizes)
2. Computes a **SHA-256 commitment** over the ship layout + a random blinding factor
3. Publishes the commitment hash — the board is now locked

#### Rounds 1+ — Gameplay Loop

Each round follows 4 steps:

| Step | Action | ZK Proof |
|------|--------|----------|
| 1 | Attacker declares target coordinate | — |
| 2 | Defender proves **hit or miss** | `hit_miss` guest program re-derives the board from the private witness, checks the commitment, and reports the result |
| 3a | If hit caused a sinking → defender proves **which ship sunk** | `ship_sunk` guest verifies all cells of a specific ship were hit |
| 3b | If hit but no sinking → defender proves **no ship fully sunk** | `no_ship_sunk` guest shows each ship has at least one surviving cell |
| 4 | Round closes, roles swap | — |

Every proof recomputes the SHA-256 commitment from the private witness and asserts it matches the Round 0 commitment — this is the chain of trust.


## Project Structure

```
├── core/                  # Shared types & logic (host + guest)
│   └── src/lib.rs         #   Ship, ShipType, Orientation, proof I/O structs,
│                          #   normalize(), canonical_preimage()
├── host/                  # Game engine (runs on host CPU)
│   └── src/
│       ├── main.rs        #   Entry point & game loop
│       ├── logic.rs       #   Round 0 placement, Round 1+ orchestration,
│       │                  #   proof generation & verification
│       ├── storage.rs     #   GameStore — in-memory state (commitments,
│       │                  #   transcript, attack log, sunk ships)
│       └── display.rs     #   CLI interface (board rendering, prompts)
│   └── tests/             #   Integration tests (25 tests)
├── methods/               # ZK guest programs (run inside zkVM)
│   └── guest/src/bin/
│       ├── validate_board.rs   # Round 0: board validity + commitment
│       ├── hit_miss.rs         # Proves attack result (hit/miss)
│       ├── ship_sunk.rs        # Proves a specific ship is fully sunk
│       └── no_ship_sunk.rs     # Proves no ship is fully sunk yet
└── plans/                 # Design documents & implementation notes
```

### Crate Roles

| Crate | Target | Role |
|-------|--------|------|
| `core` | Host + Guest | Shared types, serialization, commitment logic |
| `host` | Native CPU | Game orchestration, I/O, proof dispatch |
| `methods` | Native CPU | Build script that compiles guest binaries, exposes ELF constants & image IDs |
| `methods-guest` | `riscv32im-risc0-zkvm-elf` | ZK circuits — the code that runs inside the zkVM |

