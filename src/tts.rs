use anyhow::{Context, Result};
use qwen3_tts::inference::TTSInference;
use qwen3_tts::tensor::Device;
use std::path::Path;

/// Synthesize speech from text using Qwen3-TTS.
///
/// Returns raw f32 audio samples and the sample rate.
/// The caller is responsible for encoding the samples to the desired output format.
pub fn synthesize(
    model_dir: &str,
    text: &str,
    speaker: &str,
    language: &str,
) -> Result<(Vec<f32>, u32)> {
    // Determine device based on backend — matches qwen3_audio_api approach
    #[cfg(feature = "mlx")]
    let device = {
        qwen3_tts::backend::mlx::stream::init_mlx(true);
        tracing::info!("TTS using MLX Metal GPU");
        Device::Gpu(0)
    };

    #[cfg(feature = "tch-backend")]
    let device = {
        if tch::Cuda::is_available() {
            tracing::info!("TTS using CUDA GPU");
            Device::Gpu(0)
        } else {
            tracing::info!("TTS using CPU");
            Device::Cpu
        }
    };

    tracing::info!("Loading TTS model from: {model_dir}");
    let inference =
        TTSInference::new(Path::new(model_dir), device).context("failed to load TTS model")?;

    tracing::info!("Generating speech for: {text}");
    let (samples, sample_rate) = inference
        .generate_with_instruct(
            text, speaker, language, "",   // instruction (empty for basic generation)
            0.9,  // temperature
            50,   // top_k
            2048, // max_codes
        )
        .context("TTS generation failed")?;

    tracing::info!(
        "TTS generated {} samples at {}Hz",
        samples.len(),
        sample_rate
    );

    Ok((samples, sample_rate))
}
