use serde::{Serialize, Deserialize};


///! NOTE : adding Copy to the new types we made, would allow us to
///! forgot about the ownership rules (to some extent) and move freely

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

//? discarded afterwards.  The `Ship` struct never stores a direction.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShipType {
    //? 5 squares
    Carrier,
    //? 4 squares
    Battleship,
    //? 3 squares
    Cruiser,
    //? 3 squares
    Submarine,
    //? 2 squares
    Destroyer,
}

impl ShipType {
    
    pub fn len(self) -> u8 {
        match self {
            ShipType::Carrier => 5,
            ShipType::Battleship => 4,
            ShipType::Cruiser => 3,
            ShipType::Submarine => 3,
            ShipType::Destroyer => 2,
        }
    }

    /// A human-friendly name for error messages and debugging.
    pub fn name(self) -> &'static str {
        match self {
            ShipType::Carrier => "Carrier (5)",
            ShipType::Battleship => "Battleship (4)",
            ShipType::Cruiser => "Cruiser (3)",
            ShipType::Submarine => "Submarine (3)",
            ShipType::Destroyer => "Destroyer (2)",
        }
    }

    /// The canonical placement order used throughout the game (the same order
    /// both players always enter their ships).
    pub const ALL: [ShipType; 5] = [
        ShipType::Carrier,
        ShipType::Battleship,
        ShipType::Cruiser,
        ShipType::Submarine,
        ShipType::Destroyer,
    ];
}

// ---------------------------------------------------------------------------
// Ship
// ---------------------------------------------------------------------------


#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ship {
    pub ship_type: ShipType,
    /// Top-left row anchor, 0-indexed.
    pub row: u8,
    /// Top-left column anchor, 0-indexed.
    pub col: u8,
    pub orientation: Orientation,
}



//? This code is to generate the cells on the fly, we are using an iterator
//? and we would need 
impl Ship {
    /// Cells are derived on-the-fly
    pub fn cells(self) -> impl Iterator<Item = (u8, u8)> {
        let len = self.ship_type.len();
        //? The move here is for the closure to take ownership of the data,
        //? as we will need it after the the self would be dropped, 
        //? because the iterator would need to reference self anyways.
        (0..len).map(move |i| match self.orientation {
            Orientation::Horizontal => (self.row, self.col + i),
            Orientation::Vertical => (self.row + i, self.col),
        })
    }
}


/// Converts user input `(row, col, axis, direction)` into the canonical
/// `(anchor_row, anchor_col)` where the anchor is always the top-left corner.

pub fn normalize(row: u8, col: u8, axis: Orientation, direction: Direction, len: u8) -> (u8, u8) {
    match (axis, direction) {
        (Orientation::Horizontal, Direction::Left) => (row, col + 1 - len),
        (Orientation::Horizontal, Direction::Right) => (row, col),
        (Orientation::Vertical, Direction::Up) => (row + 1 - len, col),
        (Orientation::Vertical, Direction::Down) => (row, col),
        // Invalid combinations: Up/Down don't apply to Horizontal, Left/Right don't apply to Vertical.
        (Orientation::Horizontal, Direction::Up) | (Orientation::Horizontal, Direction::Down) => {
            panic!("normalize: Up/Down direction is invalid for a Horizontal ship")
        }
        (Orientation::Vertical, Direction::Left) | (Orientation::Vertical, Direction::Right) => {
            panic!("normalize: Left/Right direction is invalid for a Vertical ship")
        }
    }
}

// ---------------------------------------------------------------------------
// Proof I/O types
// ---------------------------------------------------------------------------

///! The private witness sent to the guest program.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BoardCommitInput {
    pub ships: [Ship; 5],
    pub blinding: [u8; 32],
}

///! The public output committed to the journal by the guest.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BoardCommitOutput {
    pub commitment: [u8; 32],
}

// ---------------------------------------------------------------------------
// Player state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Player {
    pub id: usize, // 0 or 1
    pub ships: Option<[Ship; 5]>,
}

impl Player {
    pub fn new(id: usize) -> Self {
        Player { id, ships: None }
    }
}

/// The total number of cells all 5 ships must occupy: 5+4+3+3+2 = 17.
pub const TOTAL_SHIP_CELLS: usize = 17;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ship_type_lengths() {
        assert_eq!(ShipType::Carrier.len(), 5);
        assert_eq!(ShipType::Battleship.len(), 4);
        assert_eq!(ShipType::Cruiser.len(), 3);
        assert_eq!(ShipType::Submarine.len(), 3);
        assert_eq!(ShipType::Destroyer.len(), 2);
    }

    #[test]
    fn cells_horizontal() {
        let ship = Ship {
            ship_type: ShipType::Destroyer,
            row: 2,
            col: 3,
            orientation: Orientation::Horizontal,
        };
        let cells: Vec<_> = ship.cells().collect();
        assert_eq!(cells, vec![(2, 3), (2, 4)]);
    }

    #[test]
    fn cells_vertical() {
        let ship = Ship {
            ship_type: ShipType::Cruiser,
            row: 0,
            col: 5,
            orientation: Orientation::Vertical,
        };
        let cells: Vec<_> = ship.cells().collect();
        assert_eq!(cells, vec![(0, 5), (1, 5), (2, 5)]);
    }

    #[test]
    fn normalize_left() {
        // Anchor at (2, 5), extending Left for len=3 → top-left is (2, 3)
        let (r, c) = normalize(2, 5, Orientation::Horizontal, Direction::Left, 3);
        assert_eq!((r, c), (2, 3));
    }

    #[test]
    fn normalize_up() {
        // Anchor at (5, 2), extending Up for len=4 → top-left is (2, 2)
        let (r, c) = normalize(5, 2, Orientation::Vertical, Direction::Up, 4);
        assert_eq!((r, c), (2, 2));
    }

    #[test]
    fn total_ship_cells_constant() {
        let total: usize = ShipType::ALL.iter().map(|s| s.len() as usize).sum();
        assert_eq!(total, TOTAL_SHIP_CELLS);
    }
}
