use anyhow::{Context, Result};
use qwen3_asr::inference::AsrInference;
use qwen3_asr::tensor::Device;
use std::path::Path;

/// Transcribe an audio file to text using Qwen3-ASR.
///
/// The ASR library handles any FFmpeg-compatible audio format internally
/// (wav, mp3, flac, opus, etc.), so no pre-conversion is needed.
///
/// Returns the transcribed text.
pub fn transcribe(model_dir: &str, audio_file: &str, language: Option<&str>) -> Result<String> {
    #[cfg(feature = "tch-backend")]
    let device = if tch::Cuda::is_available() {
        tracing::info!("Using CUDA device");
        Device::Gpu(0)
    } else {
        tracing::info!("Using CPU device");
        Device::Cpu
    };

    #[cfg(feature = "mlx")]
    let device = {
        qwen3_asr::backend::mlx::stream::init_mlx(true);
        tracing::info!("Using MLX Metal GPU");
        Device::Gpu(0)
    };

    tracing::info!("Loading ASR model from: {model_dir}");
    let asr =
        AsrInference::load(Path::new(model_dir), device).context("failed to load ASR model")?;

    tracing::info!("Transcribing: {audio_file}");
    let result = asr
        .transcribe(audio_file, language)
        .context("transcription failed")?;

    tracing::info!("Detected language: {}", result.language);
    Ok(result.text)
}
