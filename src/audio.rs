use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;

/// Write raw f32 audio samples to a WAV file (16-bit PCM, mono).
pub fn write_wav(samples: &[f32], sample_rate: u32, output_path: &Path) -> Result<()> {
    let num_samples = samples.len() as u32;
    let bytes_per_sample: u16 = 2; // 16-bit
    let num_channels: u16 = 1;
    let data_size = num_samples * bytes_per_sample as u32;
    let file_size = 36 + data_size; // 44 byte header - 8 byte RIFF header

    let mut file =
        std::fs::File::create(output_path).context("failed to create WAV output file")?;

    // RIFF header
    file.write_all(b"RIFF")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmt chunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // chunk size
    file.write_all(&1u16.to_le_bytes())?; // PCM format
    file.write_all(&num_channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    let byte_rate = sample_rate * num_channels as u32 * bytes_per_sample as u32;
    file.write_all(&byte_rate.to_le_bytes())?;
    let block_align = num_channels * bytes_per_sample;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&(bytes_per_sample * 8).to_le_bytes())?; // bits per sample

    // data chunk
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    // Convert f32 samples to i16 and write
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let i16_val = (clamped * 32767.0) as i16;
        file.write_all(&i16_val.to_le_bytes())?;
    }

    if !output_path.exists() {
        bail!("WAV output file was not created");
    }

    tracing::info!("WAV written to: {}", output_path.display());
    Ok(())
}

/// Encode raw f32 audio samples to an MP3 file using ffmpeg-next (libmp3lame).
///
/// Converts f32 samples (range -1.0..1.0) to i16, processes them in
/// encoder.frame_size() chunks, and writes the MP3 output file.
pub fn encode_mp3(samples: &[f32], sample_rate: u32, output_mp3: &Path) -> Result<()> {
    ffmpeg_next::init().context("failed to initialize ffmpeg")?;

    // Find MP3 encoder
    let codec = ffmpeg_next::encoder::find_by_name("libmp3lame").context(
        "MP3 encoder (libmp3lame) not found — is ffmpeg built with --enable-libmp3lame?",
    )?;

    // Create output format context
    let mut octx =
        ffmpeg_next::format::output(output_mp3).context("failed to create MP3 output context")?;

    // Create encoder context from codec, configure, then add stream
    let mut encoder = {
        // Determine encoder's preferred sample format (fallback to i16 packed)
        let default_sample_fmt =
            ffmpeg_next::format::Sample::I16(ffmpeg_next::format::sample::Type::Packed);
        let enc_sample_format = codec
            .audio()
            .ok()
            .and_then(|a| a.formats())
            .and_then(|mut f| f.next())
            .unwrap_or(default_sample_fmt);

        // Create encoder context directly from codec (stream.codec() removed in ffmpeg-next 8.0)
        let mut encoder = ffmpeg_next::codec::context::Context::new_with_codec(codec)
            .encoder()
            .audio()
            .context("failed to create audio encoder")?;

        encoder.set_rate(sample_rate as i32);
        encoder.set_channel_layout(ffmpeg_next::ChannelLayout::MONO);
        encoder.set_format(enc_sample_format);
        encoder.set_bit_rate(192_000);

        let encoder = encoder
            .open_as(codec)
            .context("failed to open MP3 encoder")?;

        // Add stream and copy encoder parameters to it
        let mut stream = octx
            .add_stream(codec)
            .context("failed to add audio stream")?;
        stream.set_parameters(&encoder);

        encoder
    };

    // Write file header
    octx.write_header().context("failed to write MP3 header")?;

    let output_stream_time_base = octx.stream(0).unwrap().time_base();

    // Get encoder's frame size (576 for MP3, default to 1024 if zero)
    let frame_size = if encoder.frame_size() > 0 {
        encoder.frame_size() as usize
    } else {
        1024
    };

    let enc_sample_format = encoder.format();
    let enc_rate = encoder.rate();

    let mut pts: i64 = 0;
    let mut offset: usize = 0;

    while offset < samples.len() {
        let chunk_len = std::cmp::min(frame_size, samples.len() - offset);
        let chunk = &samples[offset..offset + chunk_len];

        let mut frame = ffmpeg_next::frame::Audio::new(
            enc_sample_format,
            chunk_len,
            ffmpeg_next::ChannelLayout::MONO,
        );
        frame.set_rate(enc_rate);
        frame.set_pts(Some(pts));

        // Convert f32 samples to i16 and write into frame buffer
        let data = frame.data_mut(0);
        for (i, &s) in chunk.iter().enumerate() {
            let clamped = s.clamp(-1.0, 1.0);
            let i16_val = (clamped * 32767.0) as i16;
            let bytes = i16_val.to_le_bytes();
            data[i * 2] = bytes[0];
            data[i * 2 + 1] = bytes[1];
        }

        encoder
            .send_frame(&frame)
            .context("encoder send_frame failed")?;

        receive_and_write_packets(&mut encoder, &mut octx, output_stream_time_base)?;

        pts += chunk_len as i64;
        offset += chunk_len;
    }

    // Flush encoder
    encoder.send_eof().context("encoder send_eof failed")?;
    receive_and_write_packets(&mut encoder, &mut octx, output_stream_time_base)?;

    // Write file trailer
    octx.write_trailer()
        .context("failed to write MP3 trailer")?;

    if !output_mp3.exists() {
        bail!("MP3 output file was not created");
    }

    tracing::info!("MP3 written to: {}", output_mp3.display());
    Ok(())
}

/// Drain encoded packets from the encoder and write them to the output context.
fn receive_and_write_packets(
    encoder: &mut ffmpeg_next::encoder::Audio,
    octx: &mut ffmpeg_next::format::context::Output,
    time_base: ffmpeg_next::Rational,
) -> Result<()> {
    let mut encoded_packet = ffmpeg_next::Packet::empty();
    while encoder.receive_packet(&mut encoded_packet).is_ok() {
        if encoded_packet.size() == 0 {
            continue;
        }
        encoded_packet.set_stream(0);
        encoded_packet.rescale_ts(encoder.time_base(), time_base);
        encoded_packet
            .write_interleaved(octx)
            .context("failed to write encoded packet")?;
    }
    Ok(())
}
