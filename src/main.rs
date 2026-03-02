// Ensure exactly one backend is selected
#[cfg(all(feature = "tch-backend", feature = "mlx"))]
compile_error!("Features 'tch-backend' and 'mlx' are mutually exclusive. Enable only one.");

#[cfg(not(any(feature = "tch-backend", feature = "mlx")))]
compile_error!("Either 'tch-backend' or 'mlx' feature must be enabled.");

mod asr;
mod audio;
mod tts;
mod wasm_runner;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

/// Voice-to-voice processing pipeline.
///
/// Transcribes an audio file to text (ASR), runs the text through a chain of
/// WASM processing modules, then synthesizes the result back to speech (TTS).
#[derive(Parser)]
#[command(name = "voice-actions", version, about)]
struct Cli {
    /// Path to the input audio file (any format: wav, mp3, flac, opus, etc.)
    #[arg(short, long)]
    input: PathBuf,

    /// Path to the output audio file (MP3)
    #[arg(short, long)]
    output: PathBuf,

    /// Path to the Qwen3-ASR model directory
    #[arg(long)]
    asr_model: PathBuf,

    /// Path to the Qwen3-TTS model directory
    #[arg(long)]
    tts_model: PathBuf,

    /// WASM module files to chain (each must export `allocate` and `process`).
    /// Modules are executed in the order specified.
    #[arg(long = "wasm", required = true)]
    wasm_files: Vec<PathBuf>,

    /// Language hint for ASR (e.g. "en", "zh"). Auto-detected if omitted.
    #[arg(long)]
    language: Option<String>,

    /// TTS speaker name (default: "Ryan")
    #[arg(long, default_value = "Ryan")]
    speaker: String,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Initialize MLX for both crates upfront, TTS first then ASR —
    // matching qwen3_audio_api's init order.  Each crate has its own
    // DEFAULT_STREAM but they share the MLX C library's global state.
    #[cfg(feature = "mlx")]
    {
        qwen3_tts::backend::mlx::stream::init_mlx(true);
        qwen3_asr::backend::mlx::stream::init_mlx(true);
    }

    let cli = Cli::parse();

    // --- Step 1: ASR – transcribe audio to text ---
    tracing::info!("Transcribing audio: {}", cli.input.display());
    let transcribed_text = asr::transcribe(
        cli.asr_model.to_str().context("invalid ASR model path")?,
        cli.input.to_str().context("invalid input path")?,
        cli.language.as_deref(),
    )
    .context("ASR transcription failed")?;
    tracing::info!("Transcribed text: {transcribed_text}");

    // --- Step 2: WASM chain – process text through each module ---
    let wasm_paths: Vec<String> = cli
        .wasm_files
        .iter()
        .map(|p| {
            p.to_str()
                .context("invalid WASM file path")
                .map(|s| s.to_string())
        })
        .collect::<Result<_>>()?;

    tracing::info!(
        "Running WASM processing chain ({} module(s))",
        wasm_paths.len()
    );
    println!("WASM input:  {transcribed_text}");
    let processed_text = wasm_runner::run_wasm_chain(&wasm_paths, &transcribed_text)
        .context("WASM processing chain failed")?;
    println!("WASM output: {processed_text}");

    // --- Step 3: TTS – synthesize speech from processed text ---
    tracing::info!("Synthesizing speech");
    let tts_language = cli.language.as_deref().unwrap_or("english");
    let (samples, sample_rate) = tts::synthesize(
        cli.tts_model.to_str().context("invalid TTS model path")?,
        &processed_text,
        &cli.speaker,
        tts_language,
    )
    .context("TTS synthesis failed")?;
    tracing::info!(
        "TTS produced {} samples at {}Hz",
        samples.len(),
        sample_rate
    );

    // --- Step 4: Write WAV and encode MP3 ---
    let wav_output = cli.output.with_extension("wav");
    tracing::info!("Writing WAV: {}", wav_output.display());
    audio::write_wav(&samples, sample_rate, &wav_output).context("WAV encoding failed")?;

    tracing::info!("Encoding MP3: {}", cli.output.display());
    audio::encode_mp3(&samples, sample_rate, &cli.output).context("MP3 encoding failed")?;

    tracing::info!(
        "Done! Output: {} and {}",
        cli.output.display(),
        wav_output.display()
    );
    Ok(())
}
