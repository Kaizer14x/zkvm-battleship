use battleship_core::Player;



///TODO : IMPLMENTING THE PERSISTENCE LAYER OF THE GAME


/// All game state that lives in memory for one session.
///

/// The `commitments` array holds the 32-byte SHA-256 board commitment
/// produced by the `validate_board` guest program.  `None` means the player
/// has not yet completed Round 0.
pub struct GameStore {
    pub players: [Player; 2],
    pub commitments: [Option<[u8; 32]>; 2],
    pub round: u32,
}

impl GameStore {
    /// Create a fresh game with two uninitialised players.
    pub fn new() -> Self {
        GameStore {
            players: [Player::new(0), Player::new(1)],
            commitments: [None, None],
            round: 0,
        }
    }

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

    /// Returns true once both players have committed their boards.
    pub fn both_committed(&self) -> bool {
        self.commitments[0].is_some() && self.commitments[1].is_some()
    }
}
