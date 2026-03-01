use anyhow::{bail, Context, Result};
use std::path::Path;

/// Encode raw f32 audio samples to an MP3 file using ffmpeg-next (libmp3lame).
///
/// Converts f32 samples (range -1.0..1.0) to i16, processes them in
/// encoder.frame_size() chunks, and writes the MP3 output file.
pub fn encode_mp3(samples: &[f32], sample_rate: u32, output_mp3: &Path) -> Result<()> {
    ffmpeg_next::init().context("failed to initialize ffmpeg")?;

    // Find MP3 encoder
    let codec = ffmpeg_next::encoder::find_by_name("libmp3lame")
        .context("MP3 encoder (libmp3lame) not found — is ffmpeg built with --enable-libmp3lame?")?;

    // Create output format context
    let mut octx = ffmpeg_next::format::output(output_mp3)
        .context("failed to create MP3 output context")?;

    // Create encoder context from codec, configure, then add stream
    let mut encoder = {
        // Determine encoder's preferred sample format (fallback to i16 packed)
        let default_sample_fmt = ffmpeg_next::format::Sample::I16(
            ffmpeg_next::format::sample::Type::Packed,
        );
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
