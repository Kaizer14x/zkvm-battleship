use battleship_core::{AttackResult, Player, TranscriptEntry};

/// All game state that lives in memory for one session.
///
/// The `commitments` array holds the 32-byte SHA-256 board commitment
/// produced by the `validate_board` guest program.  `None` means the player
/// has not yet completed Round 0.
pub struct GameStore {
    pub players: [Player; 2],
    pub commitments: [Option<[u8; 32]>; 2],
    /// Blinding factors generated during Round 0 — required by the defender
    /// to re-derive the commitment in every subsequent proof.
    pub blindings: [Option<[u8; 32]>; 2],
    pub round: u32,

    // --- Round 1+ state ---

    /// Every coordinate ever attacked, regardless of result.
    /// Prevents duplicate attacks.
    pub attack_log: Vec<(u8, u8)>,

    /// Confirmed `(coordinate, result)` pairs, append-only.
    /// The canonical public record of the game.
    pub transcript: Vec<TranscriptEntry>,

    /// Sunk ship indices per player.  Index into `ShipType::ALL`.
    pub sunk_ships: [Vec<u8>; 2],

    /// Which player is currently attacking (0 or 1).
    pub active_attacker: usize,
}

impl GameStore {
    /// Create a fresh game with two uninitialised players.
    pub fn new() -> Self {
        GameStore {
            players: [Player::new(0), Player::new(1)],
            commitments: [None, None],
            blindings: [None, None],
            round: 0,
            attack_log: Vec::new(),
            transcript: Vec::new(),
            sunk_ships: [Vec::new(), Vec::new()],
            active_attacker: 0,
        }
    }

    // --- Round 0 setters ---

    /// Record the board commitment for `player_id` (0 or 1).
    pub fn set_commitment(&mut self, player_id: usize, commitment: [u8; 32]) {
        assert!(player_id < 2, "player_id must be 0 or 1");
        self.commitments[player_id] = Some(commitment);
    }

    /// Store the finalised ship placement for `player_id`.
    pub fn set_ships(&mut self, player_id: usize, ships: [battleship_core::Ship; 5]) {
        assert!(player_id < 2, "player_id must be 0 or 1");
        self.players[player_id].ships = Some(ships);
    }

    /// Store the blinding factor for `player_id`.
    pub fn set_blinding(&mut self, player_id: usize, blinding: [u8; 32]) {
        assert!(player_id < 2, "player_id must be 0 or 1");
        self.blindings[player_id] = Some(blinding);
    }

    /// Returns true once both players have committed their boards.
    pub fn both_committed(&self) -> bool {
        self.commitments[0].is_some() && self.commitments[1].is_some()
    }

    // --- Round 1+ helpers ---

    /// The player currently defending (the one NOT attacking).
    pub fn active_defender(&self) -> usize {
        1 - self.active_attacker
    }

    /// Has this coordinate already been attacked?
    pub fn is_already_attacked(&self, coord: (u8, u8)) -> bool {
        self.attack_log.contains(&coord)
    }

    /// Record an attack coordinate in the attack log.
    pub fn record_attack(&mut self, coord: (u8, u8)) {
        self.attack_log.push(coord);
    }

    /// Append a confirmed result to the public transcript.
    pub fn record_result(&mut self, coord: (u8, u8), result: AttackResult) {
        self.transcript.push(TranscriptEntry { coord, result });
    }

    /// Record a sunk ship for the given player.
    pub fn record_sunk(&mut self, player_id: usize, ship_index: u8) {
        assert!(player_id < 2);
        self.sunk_ships[player_id].push(ship_index);
    }

    /// Derive the hit log for a specific defender — all coordinates where the
    /// result was HIT against that defender.
    ///
    /// In the current implementation both players' attacks share one
    /// transcript, and attacks alternate.  Player 0 attacks on odd rounds
    /// (round 1, 3, …) and Player 1 attacks on even rounds (round 2, 4, …).
    /// For simplicity in CLI mode, we derive the hit log for the *current*
    /// defender by filtering all HIT entries where the defender was defending.
    ///
    /// Since the transcript is ordered and we track whose turn it is, we can
    /// reconstruct this.  For now, we keep it simple: ALL hits in the
    /// transcript (this is correct because each player only defends against
    /// attacks aimed at them, and the engine only appends entries when the
    /// attacked player is the defender).
    pub fn hit_log_for_defender(&self, _defender_id: usize) -> Vec<(u8, u8)> {
        // In a two-player alternating game, the transcript entries
        // alternate between attacks on player 0 and player 1.
        // Player 0 defends on rounds 1, 3, 5, ... (0-indexed transcript entries 0, 2, 4, ...)
        // Player 1 defends on rounds 2, 4, 6, ... (0-indexed transcript entries 1, 3, 5, ...)
        // However, for the CLI where rounds are sequential and the engine
        // tracks roles, we simply filter by defender_id.
        //
        // The transcript stores entries in order.  Even-indexed entries
        // (0, 2, 4, …) are attacks where player 0 attacked → player 1 defended.
        // Odd-indexed entries (1, 3, 5, …) are attacks where player 1 attacked → player 0 defended.
        //
        // defender_id == 0 → odd-indexed entries
        // defender_id == 1 → even-indexed entries
        self.transcript
            .iter()
            .enumerate()
            .filter(|(i, entry)| {
                entry.result == AttackResult::Hit
                    && ((_defender_id == 1 && i % 2 == 0)
                        || (_defender_id == 0 && i % 2 == 1))
            })
            .map(|(_, entry)| entry.coord)
            .collect()
    }

    /// Count total confirmed hits against a specific defender.
    pub fn hit_count_for_defender(&self, defender_id: usize) -> usize {
        self.hit_log_for_defender(defender_id).len()
    }

    /// Swap attacker and defender roles for the next round.
    pub fn swap_roles(&mut self) {
        self.active_attacker = 1 - self.active_attacker;
    }

    /// Is the game over?  True if either defender has received 17 hits
    /// (all ship cells destroyed).
    pub fn is_game_over(&self) -> Option<usize> {
        for defender_id in 0..2 {
            if self.hit_count_for_defender(defender_id) >= 17 {
                // The winner is the one who sank the other's fleet
                return Some(1 - defender_id);
            }
        }
        None
    }
}
