mod display;
mod logic;
mod storage;

use display::show_message;
use logic::round_zero;
use storage::GameStore;

fn main() {
    // Initialise tracing so risc0 proof progress lines appear on stderr.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("risc0_zkvm=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    show_message(
        "\n╔══════════════════════════════════════════╗\n\
         ║       ZK Battleship — Round 0            ║\n\
         ║  Board placement & ZK proof generation   ║\n\
         ╚══════════════════════════════════════════╝",
    );

    show_message(
        "\nRound 0: Each player places their fleet.\n\
         The board is hashed and committed using a ZK proof —\n\
         neither player can change their placement afterwards.\n",
    );

    let mut store = GameStore::new();

    // ------------------------------------------------------------------
    // Player 1 sets up their board and generates their proof.
    // ------------------------------------------------------------------
    round_zero(0, &mut store);

    show_message("\n[Player 1 board committed. Player 2, look away until your turn.]\n");
    show_message("Press ENTER to continue...");
    let _ = {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line).ok();
        line
    };

    // ------------------------------------------------------------------
    // Player 2 sets up their board and generates their proof.
    // ------------------------------------------------------------------
    round_zero(1, &mut store);

    // ------------------------------------------------------------------
    // Summary: print both board commitments publicly.
    // Both proofs are already verified inside round_zero.
    // ------------------------------------------------------------------
    show_message(
        "\n╔══════════════════════════════════════════╗\n\
         ║            Round 0 Complete              ║\n\
         ╚══════════════════════════════════════════╝",
    );
    show_message("\nPublic board commitments (SHA-256):\n");

    for player_id in 0..2 {
        let commitment =
            store.commitments[player_id].expect("commitment must be set after round_zero");
        let hex: String = commitment.iter().map(|b| format!("{:02x}", b)).collect();
        show_message(&format!("  Player {}: 0x{}", player_id + 1, hex));
    }

    show_message(
        "\nBoth boards are cryptographically committed.\n\
         Neither player can alter their fleet placement.\n\
         Round 1 (attacks) is not yet implemented.\n",
    );
}
