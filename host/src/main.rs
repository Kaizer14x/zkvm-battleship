mod display;
mod logic;
mod storage;

use display::show_message;
use logic::{play_round, round_zero};
use storage::GameStore;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("risc0_zkvm=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    show_message(
        "\n╔══════════════════════════════════════════╗\n\
         ║          ZK Battleship                   ║\n\
         ║  Board placement · ZK proof generation   ║\n\
         ╚══════════════════════════════════════════╝",
    );

    let mut store = GameStore::new();

    // ------------------------------------------------------------------
    // Round 0 — Each player commits their board under a ZK proof.
    // ------------------------------------------------------------------
    show_message(
        "\nRound 0: Each player places their fleet.\n\
         The board is hashed and committed via a ZK proof —\n\
         neither player can change their placement afterwards.\n",
    );

    round_zero(0, &mut store);

    show_message("\n[Player 1 board committed. Player 2, look away until your turn.]\n");
    wait_for_enter();

    round_zero(1, &mut store);

    // Print both public commitments.
    show_message(
        "\n╔══════════════════════════════════════════╗\n\
         ║            Round 0 Complete              ║\n\
         ╚══════════════════════════════════════════╝",
    );
    show_message("\nPublic board commitments (SHA-256):\n");
    for player_id in 0..2 {
        let c = store.commitments[player_id].expect("commitment must be set");
        let hex: String = c.iter().map(|b| format!("{:02x}", b)).collect();
        show_message(&format!("  Player {}: 0x{}", player_id + 1, hex));
    }
    show_message("\nBoth boards are cryptographically committed.\nStarting game...\n");
    wait_for_enter();

    // ------------------------------------------------------------------
    // Rounds 1+ — Main game loop.
    // ------------------------------------------------------------------
    store.round = 1;

    let winner = loop {
        match play_round(&mut store) {
            Some(winner_id) => break winner_id,
            None => {}
        }
    };

    // ------------------------------------------------------------------
    // Game over — Announce winner and print final summary.
    // ------------------------------------------------------------------
    show_message(&format!(
        "\n╔══════════════════════════════════════════╗\n\
         ║              GAME OVER                   ║\n\
         ╚══════════════════════════════════════════╝\n\
         \n  🏆 Player {} wins in {} rounds!\n",
        winner + 1,
        store.round - 1,
    ));

    show_message("  Final sunk ships:");
    display::show_sunk_summary(&store);

    let total_hits: usize = store
        .transcript
        .iter()
        .filter(|e| e.result == battleship_core::AttackResult::Hit)
        .count();
    show_message(&format!(
        "\n  Total shots fired: {}  |  Total hits: {}",
        store.attack_log.len(),
        total_hits
    ));
}

fn wait_for_enter() {
    use std::io::BufRead;
    print!("Press ENTER to continue...");
    std::io::Write::flush(&mut std::io::stdout()).ok();
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).ok();
}
