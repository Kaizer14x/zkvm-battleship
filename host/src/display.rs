use std::io::{self, BufRead, Write};

use battleship_core::{AttackResult, Direction, Orientation, Player, Ship, ShipType, TranscriptEntry};

// ---------------------------------------------------------------------------
// Basic I/O primitives
// ---------------------------------------------------------------------------

pub fn show_message(msg: &str) {
    println!("{}", msg);
}

fn read_line() -> String {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).expect("Failed to read stdin");
    line.trim().to_string()
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().ok();
    read_line()
}

// ---------------------------------------------------------------------------
// Round 0 — Board display & ship placement
// ---------------------------------------------------------------------------

/// Print a 10×10 ASCII grid showing the player's own ship placements.
pub fn show_board(player: &Player) {
    println!("\n  Player {} board:", player.id + 1);
    println!("    0 1 2 3 4 5 6 7 8 9");
    println!("   +-------------------+");

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

/// Prompt the player to place one ship.
pub fn prompt_ship_placement(ship_type: ShipType) -> (u8, u8, Orientation, Direction) {
    println!("\n--- Place your {} ---", ship_type.name());

    let row: u8 = loop {
        let s = prompt("  Anchor row (0-9): ");
        match s.parse::<u8>() {
            Ok(v) if v < 10 => break v,
            _ => println!("  Invalid: enter a number 0-9."),
        }
    };

    let col: u8 = loop {
        let s = prompt("  Anchor col (0-9): ");
        match s.parse::<u8>() {
            Ok(v) if v < 10 => break v,
            _ => println!("  Invalid: enter a number 0-9."),
        }
    };

    let orientation = loop {
        let s = prompt("  Orientation (h=horizontal / v=vertical): ");
        match s.to_lowercase().as_str() {
            "h" => break Orientation::Horizontal,
            "v" => break Orientation::Vertical,
            _ => println!("  Invalid: enter 'h' or 'v'."),
        }
    };

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

// ---------------------------------------------------------------------------
// Round 1+ — Attack display
// ---------------------------------------------------------------------------

/// Prompt the attacking player for a target coordinate.
pub fn prompt_attack(attacker_id: usize) -> (u8, u8) {
    println!("\n  Player {} — choose your target:", attacker_id + 1);

    let row: u8 = loop {
        let s = prompt("  Target row (0-9): ");
        match s.parse::<u8>() {
            Ok(v) if v < 10 => break v,
            _ => println!("  Invalid: enter a number 0-9."),
        }
    };

    let col: u8 = loop {
        let s = prompt("  Target col (0-9): ");
        match s.parse::<u8>() {
            Ok(v) if v < 10 => break v,
            _ => println!("  Invalid: enter a number 0-9."),
        }
    };

    (row, col)
}

/// Announce the verified hit/miss result.
pub fn show_attack_result(coord: (u8, u8), result: AttackResult) {
    match result {
        AttackResult::Hit => println!("\n  💥 HIT at ({}, {})!", coord.0, coord.1),
        AttackResult::Miss => println!("\n  〇 MISS at ({}, {}).", coord.0, coord.1),
    }
}

/// Display the attacker's shot board: what they know about the opponent's grid.
///
/// Legend:  `.` unknown  `X` confirmed hit  `O` confirmed miss
pub fn show_shot_board(attacker_id: usize, transcript: &[TranscriptEntry]) {
    // Determine which transcript entries belong to this attacker.
    // Attacker 0's entries are at even indices (0, 2, 4, …).
    // Attacker 1's entries are at odd  indices (1, 3, 5, …).
    let mut hits = [[false; 10]; 10];
    let mut misses = [[false; 10]; 10];

    for (i, entry) in transcript.iter().enumerate() {
        let this_attackers_shot = (attacker_id == 0 && i % 2 == 0)
            || (attacker_id == 1 && i % 2 == 1);
        if this_attackers_shot {
            let (r, c) = (entry.coord.0 as usize, entry.coord.1 as usize);
            match entry.result {
                AttackResult::Hit => hits[r][c] = true,
                AttackResult::Miss => misses[r][c] = true,
            }
        }
    }

    println!("\n  Player {} — shot board:", attacker_id + 1);
    println!("    0 1 2 3 4 5 6 7 8 9");
    println!("   +-------------------+");
    for row in 0..10usize {
        print!(" {} |", row);
        for col in 0..10usize {
            let ch = if hits[row][col] {
                'X'
            } else if misses[row][col] {
                'O'
            } else {
                '.'
            };
            print!(" {}", ch);
        }
        println!(" |");
    }
    println!("   +-------------------+");
}

// ---------------------------------------------------------------------------
// Round 1+ — Sinking announcements
// ---------------------------------------------------------------------------

/// Announce a ship has been sunk.
pub fn show_sinking_announcement(defender_id: usize, ship_type: ShipType) {
    println!(
        "\n  🚢 Player {}'s {} has been sunk!",
        defender_id + 1,
        ship_type.name()
    );
}

/// Show the current tally of sunk ships for both players.
pub fn show_sunk_summary(store: &crate::storage::GameStore) {
    println!("\n  --- Ships sunk so far ---");
    for player_id in 0..2 {
        let sunk = &store.sunk_ships[player_id];
        if sunk.is_empty() {
            println!("  Player {}: none", player_id + 1);
        } else {
            let names: Vec<&str> = sunk
                .iter()
                .map(|&idx| ShipType::ALL[idx as usize].name())
                .collect();
            println!("  Player {}: {}", player_id + 1, names.join(", "));
        }
    }
}
