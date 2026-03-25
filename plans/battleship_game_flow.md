# ZK Battleship — Game Flow & Proof Mechanics

> Scope: Everything after ship placement. Covers grid verification, round structure,
> all proofs, data logs, turn enforcement, and winning condition.
> Conceptual only — no code.

---

## 1. Transition from Round 0 — Grid Verification

Before play begins, the engine must verify that both players committed to a valid board.
Each player submits a board validity receipt generated in Round 0.

The engine does two things with each receipt:

**Step 1 — Cryptographic verification.**
The engine verifies the receipt against the known guest program ID. This confirms the
receipt was produced by the correct, unmodified guest program and that all internal
assertions passed. If verification fails, the player is rejected from the game.

**Step 2 — Journal extraction and storage.**
The engine reads the public commitment C from the journal of each receipt and stores it.
These two commitments become the anchors for every subsequent proof in the game.
Any future proof that does not match the stored commitment for its respective player
is immediately rejected.

From this point forward, neither player can change their board. The commitment makes
this mathematically impossible.

---

## 2. Data Logs — What Exists, Who Owns It, Where It Comes From

Three logs are maintained throughout the game. Their ownership and derivation rules
are strict and must not be violated.

### 2.1 The Attack Log

- **Contains:** Every coordinate ever targeted, regardless of result.
- **Purpose:** Prevents a player from attacking the same cell twice.
- **Who maintains it:** The engine, publicly.
- **When updated:** Immediately when an attack is declared, before any proof is generated.
- **Rule:** If an attack coordinate already exists in this log, the engine rejects the
  move outright. No proof is requested or generated.

### 2.2 The Public Transcript

- **Contains:** Every `(coordinate, result)` pair, in order, for confirmed attacks.
- **Purpose:** The canonical public record of the game. The source of truth for all proofs.
- **Who maintains it:** The engine, publicly. Append-only.
- **When updated:** After the hit/miss proof for that attack is verified successfully.
- **Rule:** Only the engine appends to this. The defender never touches it directly.

### 2.3 The Hit Log

- **Contains:** Every `(coordinate)` where the result was HIT.
- **Purpose:** Input for sinking detection and the sinking proof.
- **Who maintains it:** Nobody privately. It is derived on demand by filtering the
  public transcript for entries where result is HIT.
- **Critical rule:** The defender does not maintain this log. It is reconstructed
  from the public transcript by anyone who needs it — the engine, the defender when
  constructing a proof, or any observer. This prevents the defender from omitting hits.

---

## 3. Round Structure

Every round after Round 0 follows a strict sequence. No step can be skipped.
The engine enforces this sequence and rejects any out-of-order action.

```
Step 1 — Attack Declaration
Step 2 — Hit/Miss Proof
Step 3 — Sinking Declaration (mandatory after every hit)
Step 4 — Round Close
```

Each round has a round number, starting at 1. Players alternate attacking.
The round number is part of every proof's public output so the engine can verify
correct sequencing.

---

## 4. Step 1 — Attack Declaration

The attacker declares a coordinate `(ax, ay)`. This is a plain public action.
No proof is attached to it.

The engine performs two checks before proceeding:

- The coordinate is within the 10×10 grid.
- The coordinate does not already appear in the attack log.

If either check fails, the move is rejected and the attacker must declare again.
If both pass, the coordinate is immediately added to the attack log and the round
proceeds to Step 2.

**Why the engine adds to the attack log before the proof:**
The attack has been declared publicly. Adding it immediately prevents the attacker
from retracting the declaration if the result turns out unfavorable.

---

## 5. Step 2 — Hit/Miss Proof

This is the defender's responsibility. The defender holds the private witness
and is therefore the only party who can generate this proof.

### What is being proven

A single claim: *"The attack at `(ax, ay)` against my committed board, which I
committed to in Round 0, results in HIT or MISS."*

Both outcomes use the same circuit. The result is a public boolean in the journal.

### Private inputs the defender provides

- The 5 ship descriptors `(row, col, orientation)`.
- The blinding factor.
- The current attack coordinate.

### Public inputs the circuit receives

- The stored commitment C for this player.
- The attack coordinate `(ax, ay)`.
- The current round number.

### What the circuit checks internally

- The commitment recomputed from the witness matches C. This is the chain-of-trust
  check. It proves the defender is answering about the board they committed to in
  Round 0, not a different board.
- For each ship, the circuit derives all cells from its descriptor and checks whether
  `(ax, ay)` matches any of them.
- The result is aggregated: if any ship contains `(ax, ay)`, the result is HIT.
  Otherwise it is MISS.

### What the journal contains (public)

- The commitment C.
- The attack coordinate.
- The result: HIT or MISS.
- The round number.

### What the engine checks after receiving the receipt

- The receipt verifies against the hit/miss guest program ID.
- The commitment in the journal matches the stored commitment for this defender.
- The coordinate in the journal matches the declared attack for this round.
- The round number in the journal matches the current round.

If all checks pass, the engine appends `(coordinate, result)` to the public transcript
and proceeds to Step 3.

---

## 6. Step 3 — Sinking Declaration

This step is **mandatory after every HIT**. It is skipped only on a MISS.

The defender must declare one of two things and prove it:

- **Declaration A:** A specific ship has sunk.
- **Declaration B:** No ship has fully sunk yet.

The engine does not proceed to Step 4 until a valid proof for one of these
declarations is received. This is the mechanism that prevents the defender from
silently absorbing hits and never announcing a sinking.

---

### 6.1 Declaration A — A Ship Has Sunk

#### What is being proven

*"Every cell of ship X has been hit by a distinct shot in the public transcript,
against my committed board."*

#### How the hit log is used here

The defender derives the hit log by filtering the public transcript. This filtered
log is the input to the sinking proof. Because it is derived from the public
transcript — not maintained privately — the defender cannot omit or alter any entry.

#### What the defender provides as private inputs

- The 5 ship descriptors and blinding factor.
- The index of the ship being declared sunk (0 through 4).
- For each cell of that ship, the index into the hit log where that cell appears.
  These are called hit indices. There are as many as the ship's length.

#### What the circuit checks internally

- Commitment recomputation matches C.
- The declared ship index is valid (0 to 4).
- For each cell of the ship, derived from its descriptor:
  - The hit log entry at the provided index contains the exact coordinate of that cell.
- All hit indices are distinct. This prevents the same shot from being counted twice
  across different cells of the same ship.

#### What the journal contains (public)

- The commitment C.
- The ship index (which identifies the type unambiguously via the fixed-order array).
- The length of the public transcript at the time of proof. This timestamps the proof
  and prevents it from being reused in a later round.

#### What the engine does after verification

- Verifies the receipt.
- Checks C matches stored commitment.
- Checks transcript length matches current length.
- Records this ship index in the list of sunk ships for this player.
- Announces the sinking publicly (ship type is now revealed).

---

### 6.2 Declaration B — No Ship Has Sunk Yet

#### What is being proven

*"For each of my 5 ships, at least one of its cells has not yet been hit."*

This is a proof of a negative, reframed as a proof of existence:
"I can show you one surviving cell per ship." If every ship has a survivor, no ship
has fully sunk.

#### What the defender provides as private inputs

- The 5 ship descriptors and blinding factor.
- For each ship, the index of one of its cells that has not yet been hit.
  This is called the surviving cell index. It points to a position within the ship
  (0 to length-1), not into the transcript.

#### What the circuit checks internally

- Commitment recomputation matches C.
- For each ship:
  - The surviving cell index is within the valid range for that ship's length.
  - The coordinate of that cell, derived from the ship descriptor and the index,
    does not appear anywhere in the hit log.

#### What the journal contains (public)

- The commitment C.
- The current transcript length. Timestamps the proof.

#### What the engine does after verification

- Verifies the receipt.
- Checks C matches stored commitment.
- Checks transcript length matches current length.
- Proceeds to Step 4 with no sinking recorded.

#### What this proof reveals

The verifier learns only: each ship has at least one unhit cell. The verifier does
not learn which cell was chosen as the survivor, how many cells have been hit on each
ship, or where any ship is positioned. The surviving cell index is entirely private.

---

## 7. Step 4 — Round Close

The engine checks the winning condition before opening the next round.

**Winning condition:** The hit log contains 17 entries.

Since the attack log prevents double-hitting the same cell, and the public transcript
is the tamper-evident source of truth, 17 confirmed hits means every one of the 17
ship cells has been hit exactly once. All ships are necessarily sunk.

No additional proof is required for the winning condition. The 17 entries in the hit
log, derived from the verified public transcript, are the proof.

The engine announces the winner. The game ends.

If the winning condition is not met, the round number increments, players swap roles
(attacker becomes defender, defender becomes attacker), and the sequence returns to
Step 1.

---

## 8. Turn Enforcement

Turn enforcement is a protocol concern, not a circuit concern. The engine owns this.

- The engine tracks whose turn it is to attack.
- A player cannot submit an attack if it is not their turn.
- A player cannot submit a sinking declaration for a hit that belongs to a round
  where they were the attacker, not the defender.
- The round number embedded in every proof's journal prevents proofs from being
  submitted out of sequence or reused across rounds.

For the CLI phase, the engine is trusted to enforce this correctly. For the on-chain
phase, turn enforcement must move inside the circuits.

---

## 9. The Commitment Chain — How All Proofs Connect

Every single proof in the game carries C in its journal. The engine checks C against
its stored value after every verification. This single check is what ties the entire
game together.

```
Round 0:    Defender commits → C is stored by engine
Round N:    Hit/Miss proof carries C → engine checks it matches
            Sinking proof carries C → engine checks it matches
            No-sinking proof carries C → engine checks it matches
```

Without this chain, a malicious defender could generate valid proofs against a
different board in later rounds. With it, every proof is cryptographically bound
to the original placement. The board cannot change mid-game. This is not a
gentleman's agreement — it is mathematically enforced.

---

## 10. What Each Party Sees at Any Point in the Game

```
Attacker sees:
  All declared attacks (attack log)
  All hit/miss results (public transcript)
  All ship types that have been announced sunk
  Round numbers
  Both players' commitments C

Defender sees:
  Everything the attacker sees
  Their own private board (ships, blinding factor)

Observer sees:
  Everything the attacker sees
  All receipts and their journals
  Cannot determine ship positions from any of the above

Nobody (including the engine) sees:
  The actual ship positions of either player
  Which cells of a non-sunk ship have been hit
  The surviving cell chosen for Declaration B proofs
```
