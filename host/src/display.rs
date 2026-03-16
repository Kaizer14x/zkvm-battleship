use std::io::{self, BufRead, Write};

use battleship_core::{Direction, Orientation, Player, Ship, ShipType};



// GENERATED CODE —, ITS A BOILERPLATE FOR THE DISPLAY MODULE.
//TODO : MAKING A BEAUTIFUL DISPLAY



/// Print a single message line to stdout.
pub fn show_message(msg: &str) {
    println!("{}", msg);
}

/// Read one trimmed line from stdin.  Retries on I/O error.
fn read_line() -> String {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .expect("Failed to read stdin");
    line.trim().to_string()
}

/// Flush stdout so prompts appear before blocking on input.
fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().ok();
    read_line()
}

// ---------------------------------------------------------------------------
// Board display
// ---------------------------------------------------------------------------

/// Print a 10×10 ASCII grid showing the player's own ship placements.
///
/// Legend:
///   `.` — empty cell
///   `S` — occupied by a ship
pub fn show_board(player: &Player) {
    println!("\n  Player {} board:", player.id + 1);
    println!("    0 1 2 3 4 5 6 7 8 9");
    println!("   +-------------------+");

    // Build an occupancy map from the player's ships.
    let mut grid = [[false; 10]; 10];
    if let Some(ships) = &player.ships {
        for ship in ships.iter() {
            for (r, c) in ship.cells() {
                grid[r as usize][c as usize] = true;
            }
        }
    }

    for row in 0..10usize {
        print!(" {} |", row);
        for col in 0..10usize {
            let ch = if grid[row][col] { 'S' } else { '.' };
            print!(" {}", ch);
        }
        println!(" |");
    }
    println!("   +-------------------+\n");
}

// ---------------------------------------------------------------------------
// Ship placement prompt
// ---------------------------------------------------------------------------

/// Prompt the player to place one ship.
///
/// Returns `(anchor_row, anchor_col, orientation, direction)` — still in raw
/// user-input form.  The caller (logic.rs) is responsible for normalisation.
///
/// Inputs accepted:
///   row / col : 0-9
///   orientation: h / H  →  Horizontal
///                v / V  →  Vertical
///   direction:  r / R   →  Right  (for Horizontal)
///               l / L   →  Left   (for Horizontal)
///               d / D   →  Down   (for Vertical)
///               u / U   →  Up     (for Vertical)
pub fn prompt_ship_placement(ship_type: ShipType) -> (u8, u8, Orientation, Direction) {
    println!("\n--- Place your {} ---", ship_type.name());

    // Row
    let row: u8 = loop {
        let s = prompt("  Anchor row (0-9): ");
        match s.parse::<u8>() {
            Ok(v) if v < 10 => break v,
            _ => println!("  Invalid: enter a number 0-9."),
        }
    };

    // Column
    let col: u8 = loop {
        let s = prompt("  Anchor col (0-9): ");
        match s.parse::<u8>() {
            Ok(v) if v < 10 => break v,
            _ => println!("  Invalid: enter a number 0-9."),
        }
    };

    // Orientation
    let orientation = loop {
        let s = prompt("  Orientation (h=horizontal / v=vertical): ");
        match s.to_lowercase().as_str() {
            "h" => break Orientation::Horizontal,
            "v" => break Orientation::Vertical,
            _ => println!("  Invalid: enter 'h' or 'v'."),
        }
    };

    // Direction — must be consistent with the chosen orientation
    let direction = loop {
        let hint = match orientation {
            Orientation::Horizontal => "r=right / l=left",
            Orientation::Vertical => "d=down / u=up",
        };
        let s = prompt(&format!("  Direction ({}): ", hint));
        match (orientation, s.to_lowercase().as_str()) {
            (Orientation::Horizontal, "r") => break Direction::Right,
            (Orientation::Horizontal, "l") => break Direction::Left,
            (Orientation::Vertical, "d") => break Direction::Down,
            (Orientation::Vertical, "u") => break Direction::Up,
            _ => println!("  Invalid direction for {:?} ship.", orientation),
        }
    };

    (row, col, orientation, direction)
}

// ---------------------------------------------------------------------------
// Ship placement summary
// ---------------------------------------------------------------------------

/// Print a confirmation line for a placed ship.
pub fn show_ship_placed(ship: &Ship) {
    println!(
        "  -> {} placed at ({}, {}) {:?}",
        ship.ship_type.name(),
        ship.row,
        ship.col,
        ship.orientation
    );
}
