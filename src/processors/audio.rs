use anyhow::{Context, Result};
use hound::{WavSpec, WavWriter};
use std::fs::File;
use std::path::Path;
use std::time::Instant;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

use super::ProcessingStats;

/// Audio output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Ogg,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::Ogg
    }
}

/// Configuration for audio processing
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub output_format: AudioFormat,
    /// Quality for Vorbis encoding (0.0 to 1.0, where 0.5 is ~128kbps)
    pub quality: f32,
    /// Target sample rate (None = keep original)
    pub sample_rate: Option<u32>,
    /// Normalize audio volume
    pub normalize: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_format: AudioFormat::Ogg,
            quality: 0.5,
            sample_rate: None,
            normalize: false,
        }
    }
}

/// Process an audio file
pub fn process_audio(
    input: &Path,
    output: &Path,
    config: &AudioConfig,
) -> Result<ProcessingStats> {
    let start = Instant::now();
    let original_size = std::fs::metadata(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?
        .len();

    // Create output directory if needed
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Decode input audio
    let audio_data = decode_audio(input)?;

    // Apply normalization if requested
    let audio_data = if config.normalize {
        normalize_audio(audio_data)
    } else {
        audio_data
    };

    // Resample if needed
    let audio_data = if let Some(target_rate) = config.sample_rate {
        if audio_data.sample_rate != target_rate {
            resample_audio(audio_data, target_rate)?
        } else {
            audio_data
        }
    } else {
        audio_data
    };

    // Encode to output format
    match config.output_format {
        AudioFormat::Wav => encode_wav(&audio_data, output)?,
        AudioFormat::Ogg => encode_ogg(&audio_data, output, config.quality)?,
    }

    let output_size = std::fs::metadata(output)
        .with_context(|| format!("Failed to read output file: {}", output.display()))?
        .len();

    let processing_time_ms = start.elapsed().as_millis() as u64;

    Ok(ProcessingStats {
        original_size,
        output_size,
        processing_time_ms,
    })
}

/// Decoded audio data
#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub channels: u32,
    pub sample_rate: u32,
}

impl AudioData {
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples.len() as f64 / (self.sample_rate as f64 * self.channels as f64)
    }
}

/// Decode an audio file using Symphonia
fn decode_audio(path: &Path) -> Result<AudioData> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open audio file: {}", path.display()))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .with_context(|| format!("Failed to probe audio format: {}", path.display()))?;

    let mut format = probed.format;

    let track = format.tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("No audio track found in file"))?;

    let codec_params = &track.codec_params;
    let channels = codec_params.channels.map(|c| c.count() as u32).unwrap_or(2);
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);

    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &DecoderOptions::default())
        .with_context(|| "Failed to create audio decoder")?;

    let track_id = track.id;
    let mut samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet)?;
        append_samples(&decoded, &mut samples);
    }

    Ok(AudioData {
        samples,
        channels,
        sample_rate,
    })
}

fn append_samples(buffer: &AudioBufferRef, samples: &mut Vec<f32>) {
    match buffer {
        AudioBufferRef::F32(buf) => {
            for plane in buf.planes().planes() {
                samples.extend_from_slice(plane);
            }
        }
        AudioBufferRef::S16(buf) => {
            for plane in buf.planes().planes() {
                samples.extend(plane.iter().map(|&s| s as f32 / 32768.0));
            }
        }
        AudioBufferRef::S32(buf) => {
            for plane in buf.planes().planes() {
                samples.extend(plane.iter().map(|&s| s as f32 / 2147483648.0));
            }
        }
        AudioBufferRef::U8(buf) => {
            for plane in buf.planes().planes() {
                samples.extend(plane.iter().map(|&s| (s as f32 - 128.0) / 128.0));
            }
        }
        _ => {
            // For other formats, try to convert
            let spec = buffer.spec();
            let frames = buffer.frames();
            for frame in 0..frames {
                for ch in 0..spec.channels.count() {
                    // Default to silence for unsupported formats
                    samples.push(0.0);
                }
            }
        }
    }
}

/// Normalize audio to target peak level
fn normalize_audio(mut audio: AudioData) -> AudioData {
    if audio.samples.is_empty() {
        return audio;
    }

    // Find peak
    let peak = audio.samples.iter()
        .map(|s| s.abs())
        .fold(0.0f32, |a, b| a.max(b));

    if peak > 0.0 && peak != 1.0 {
        let target_peak = 0.95; // Leave some headroom
        let gain = target_peak / peak;

        for sample in &mut audio.samples {
            *sample *= gain;
        }
    }

    audio
}

/// Simple resampling (linear interpolation)
fn resample_audio(audio: AudioData, target_rate: u32) -> Result<AudioData> {
    if audio.sample_rate == target_rate {
        return Ok(audio);
    }

    let ratio = target_rate as f64 / audio.sample_rate as f64;
    let channels = audio.channels as usize;
    let input_frames = audio.samples.len() / channels;
    let output_frames = (input_frames as f64 * ratio).ceil() as usize;

    let mut output = Vec::with_capacity(output_frames * channels);

    for frame in 0..output_frames {
        let src_pos = frame as f64 / ratio;
        let src_frame = src_pos.floor() as usize;
        let frac = (src_pos - src_frame as f64) as f32;

        for ch in 0..channels {
            let idx0 = src_frame * channels + ch;
            let idx1 = ((src_frame + 1).min(input_frames - 1)) * channels + ch;

            let s0 = audio.samples.get(idx0).copied().unwrap_or(0.0);
            let s1 = audio.samples.get(idx1).copied().unwrap_or(0.0);

            // Linear interpolation
            output.push(s0 + (s1 - s0) * frac);
        }
    }

    Ok(AudioData {
        samples: output,
        channels: audio.channels,
        sample_rate: target_rate,
    })
}

/// Encode audio to WAV format
fn encode_wav(audio: &AudioData, output: &Path) -> Result<()> {
    let spec = WavSpec {
        channels: audio.channels as u16,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(output, spec)
        .with_context(|| format!("Failed to create WAV file: {}", output.display()))?;

    for &sample in &audio.samples {
        // Convert f32 to i16
        let s = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(s)?;
    }

    writer.finalize()?;
    Ok(())
}

/// Encode audio to OGG Vorbis format
fn encode_ogg(audio: &AudioData, output: &Path, quality: f32) -> Result<()> {
    let output_file = File::create(output)
        .with_context(|| format!("Failed to create OGG file: {}", output.display()))?;

    // Deinterleave samples for vorbis encoder
    let channels = audio.channels as usize;
    let frames = audio.samples.len() / channels;

    let mut channel_data: Vec<Vec<f32>> = vec![Vec::with_capacity(frames); channels];

    for (i, &sample) in audio.samples.iter().enumerate() {
        let ch = i % channels;
        channel_data[ch].push(sample);
    }

    // Create encoder using builder
    let sample_rate = std::num::NonZeroU32::new(audio.sample_rate)
        .ok_or_else(|| anyhow::anyhow!("Invalid sample rate: 0"))?;
    let num_channels = std::num::NonZeroU8::new(audio.channels as u8)
        .ok_or_else(|| anyhow::anyhow!("Invalid channel count: 0"))?;

    let mut encoder = VorbisEncoderBuilder::new(
        sample_rate,
        num_channels,
        output_file,
    )
    .map_err(|e| anyhow::anyhow!("Failed to create Vorbis encoder builder: {:?}", e))?
    .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr { target_quality: quality })
    .build()
    .map_err(|e| anyhow::anyhow!("Failed to build Vorbis encoder: {:?}", e))?;

    // Encode in chunks
    const CHUNK_SIZE: usize = 4096;
    let mut pos = 0;

    while pos < frames {
        let end = (pos + CHUNK_SIZE).min(frames);

        // Prepare channel slices for this chunk
        let chunk_refs: Vec<&[f32]> = channel_data.iter()
            .map(|ch| &ch[pos..end])
            .collect();

        encoder.encode_audio_block(&chunk_refs)
            .map_err(|e| anyhow::anyhow!("Failed to encode audio block: {:?}", e))?;

        pos = end;
    }

    encoder.finish()
        .map_err(|e| anyhow::anyhow!("Failed to finalize Vorbis file: {:?}", e))?;

    Ok(())
}

/// Get audio file information
pub fn get_audio_info(path: &Path) -> Result<AudioInfo> {
    let audio = decode_audio(path)?;

    Ok(AudioInfo {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        duration_secs: audio.duration_secs(),
        format: detect_audio_format(path),
    })
}

/// Audio file information
#[derive(Debug, Clone)]
pub struct AudioInfo {
    pub channels: u32,
    pub sample_rate: u32,
    pub duration_secs: f64,
    pub format: String,
}

fn detect_audio_format(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_else(|| "Unknown".to_string())
}
