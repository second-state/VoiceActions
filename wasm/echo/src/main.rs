//! Echo WASM module for voice-actions.
//!
//! Build with:
//!   cargo build --target wasm32-wasip1 --release
//!
//! This module passes the input text through unchanged.
//! It reads from `input.txt` and writes to `output.txt` in the
//! pre-opened working directory.

fn main() {
    let input = std::fs::read_to_string("input.txt").expect("failed to read input.txt");
    std::fs::write("output.txt", input).expect("failed to write output.txt");
}
