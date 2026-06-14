use std::path::PathBuf;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

fn parse_highpass_hz(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if v > 0.0 && v <= 20000.0 {
        Ok(v)
    } else {
        Err("must be between 0 and 20000 Hz".to_string())
    }
}

fn parse_comp_threshold_db(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if (-60.0..=-1.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be between -60 and -1 dBFS".to_string())
    }
}

fn parse_comp_ratio(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if (1.0..=20.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be between 1.0 and 20.0".to_string())
    }
}

fn parse_attack_ms(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if (0.1..=500.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be between 0.1 and 500 ms".to_string())
    }
}

fn parse_release_ms(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if (1.0..=2000.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be between 1 and 2000 ms".to_string())
    }
}

fn parse_db_ceiling(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if (-20.0..=0.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be between -20 and 0 dBFS".to_string())
    }
}

#[derive(clap::Args)]
pub struct Args {
    /// Input WAV file
    input: PathBuf,

    /// Output file (defaults to overwriting input)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// High-pass filter cutoff in Hz (0–20000)
    #[arg(long, default_value_t = 30.0, value_parser = parse_highpass_hz)]
    highpass_hz: f32,

    /// Compressor threshold in dBFS (-60 to -1)
    #[arg(long, default_value_t = -18.0, value_parser = parse_comp_threshold_db)]
    comp_threshold_db: f32,

    /// Compressor ratio, e.g. 3.0 = 3:1 (1.0–20.0)
    #[arg(long, default_value_t = 3.0, value_parser = parse_comp_ratio)]
    comp_ratio: f32,

    /// Compressor attack in milliseconds (0.1–500)
    #[arg(long, default_value_t = 10.0, value_parser = parse_attack_ms)]
    comp_attack_ms: f32,

    /// Compressor release in milliseconds (1–2000)
    #[arg(long, default_value_t = 100.0, value_parser = parse_release_ms)]
    comp_release_ms: f32,

    /// Limiter ceiling in dBFS (-20 to 0)
    #[arg(long, default_value_t = -1.0, value_parser = parse_db_ceiling)]
    limiter_ceiling_db: f32,

    /// Output peak target in dBFS (-20 to 0)
    #[arg(short, long, default_value_t = -1.0, value_parser = parse_db_ceiling)]
    target_db: f32,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let output_path = args.output.unwrap_or_else(|| args.input.clone());

    let mut reader = WavReader::open(&args.input)?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate as f32;

    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|s| s as f32 / max_val)
                .collect()
        }
    };

    let samples = highpass_filter(&samples, channels, sample_rate, args.highpass_hz);
    let samples = compress(
        &samples,
        channels,
        sample_rate,
        args.comp_threshold_db,
        args.comp_ratio,
        args.comp_attack_ms,
        args.comp_release_ms,
    );
    let samples = limit(&samples, channels, sample_rate, args.limiter_ceiling_db);

    let target_linear = 10f32.powf(args.target_db / 20.0);
    let peak = samples.iter().map(|s| s.abs()).fold(0f32, f32::max);
    if peak == 0.0 {
        return Err("Audio is silent; cannot master".into());
    }
    let norm_gain = target_linear / peak;
    let samples: Vec<f32> = samples
        .iter()
        .map(|s| (s * norm_gain).clamp(-1.0, 1.0))
        .collect();

    let tmp_path = output_path.with_extension("klang_tmp.wav");
    {
        let mut writer = WavWriter::create(&tmp_path, spec)?;
        match spec.sample_format {
            SampleFormat::Float => {
                for s in &samples {
                    writer.write_sample(*s)?;
                }
            }
            SampleFormat::Int => {
                let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
                for s in &samples {
                    write_int_sample(
                        &mut writer,
                        (s * max_val).clamp(-max_val, max_val - 1.0) as i64,
                        spec,
                    )?;
                }
            }
        }
        writer.finalize()?;
    }
    std::fs::rename(&tmp_path, &output_path)?;

    println!("Mastered:");
    println!("  High-pass:   {} Hz", args.highpass_hz);
    println!(
        "  Compression: threshold {:.1} dBFS, ratio {:.1}:1, attack {:.0}ms, release {:.0}ms",
        args.comp_threshold_db, args.comp_ratio, args.comp_attack_ms, args.comp_release_ms
    );
    println!("  Limiter:     ceiling {:.1} dBFS", args.limiter_ceiling_db);
    println!(
        "  Normalized:  peak {:.2} dBFS → {:.2} dBFS",
        20.0 * peak.log10(),
        args.target_db
    );

    Ok(())
}

/// Second-order Butterworth high-pass filter (biquad IIR), applied per channel.
fn highpass_filter(samples: &[f32], channels: usize, sample_rate: f32, cutoff_hz: f32) -> Vec<f32> {
    use std::f32::consts::PI;

    let w0 = 2.0 * PI * cutoff_hz / sample_rate;
    let alpha = w0.sin() / (2.0 * std::f32::consts::SQRT_2); // Q = 1/sqrt(2) (Butterworth)
    let cos_w0 = w0.cos();
    let a0 = 1.0 + alpha;

    let b0 = (1.0 + cos_w0) / 2.0 / a0;
    let b1 = -(1.0 + cos_w0) / a0;
    let b2 = (1.0 + cos_w0) / 2.0 / a0;
    let a1 = -2.0 * cos_w0 / a0;
    let a2 = (1.0 - alpha) / a0;

    // Per-channel state: [x1, x2, y1, y2]
    let mut state = vec![[0f32; 4]; channels];
    let mut output = vec![0f32; samples.len()];

    for (i, &x) in samples.iter().enumerate() {
        let ch = i % channels;
        let [x1, x2, y1, y2] = state[ch];
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        state[ch] = [x, x1, y, y1];
        output[i] = y;
    }

    output
}

/// Linked-stereo peak compressor with attack/release envelope follower.
fn compress(
    samples: &[f32],
    channels: usize,
    sample_rate: f32,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
) -> Vec<f32> {
    let attack_coeff = (-1.0 / (attack_ms * 0.001 * sample_rate)).exp();
    let release_coeff = (-1.0 / (release_ms * 0.001 * sample_rate)).exp();
    let threshold_linear = 10f32.powf(threshold_db / 20.0);

    let mut envelope = 0f32;
    let frames = samples.len() / channels;
    let mut output = vec![0f32; samples.len()];

    for f in 0..frames {
        let peak = (0..channels)
            .map(|c| samples[f * channels + c].abs())
            .fold(0f32, f32::max);

        let coeff = if peak > envelope {
            attack_coeff
        } else {
            release_coeff
        };
        envelope = coeff * envelope + (1.0 - coeff) * peak;

        let gain = if envelope > threshold_linear {
            let level_db = 20.0 * envelope.log10();
            let gain_db = (1.0 / ratio - 1.0) * (level_db - threshold_db);
            10f32.powf(gain_db / 20.0)
        } else {
            1.0
        };

        for c in 0..channels {
            output[f * channels + c] = samples[f * channels + c] * gain;
        }
    }

    output
}

/// Linked-stereo brickwall limiter with fast attack and moderate release.
fn limit(samples: &[f32], channels: usize, sample_rate: f32, ceiling_db: f32) -> Vec<f32> {
    let ceiling = 10f32.powf(ceiling_db / 20.0);
    let attack_coeff = (-1.0 / (0.1 * 0.001 * sample_rate)).exp(); // 0.1ms
    let release_coeff = (-1.0 / (50.0 * 0.001 * sample_rate)).exp(); // 50ms

    let frames = samples.len() / channels;
    let mut gain = 1f32;
    let mut output = vec![0f32; samples.len()];

    for f in 0..frames {
        let peak = (0..channels)
            .map(|c| samples[f * channels + c].abs())
            .fold(0f32, f32::max);

        let target_gain = if peak > ceiling { ceiling / peak } else { 1.0 };

        gain = if target_gain < gain {
            attack_coeff * gain + (1.0 - attack_coeff) * target_gain
        } else {
            release_coeff * gain + (1.0 - release_coeff) * target_gain
        };

        // Hard clip to ceiling as a safety net
        let effective_gain = if peak > ceiling {
            gain.min(ceiling / peak)
        } else {
            gain
        };

        for c in 0..channels {
            output[f * channels + c] = samples[f * channels + c] * effective_gain;
        }
    }

    output
}

fn write_int_sample(
    writer: &mut WavWriter<std::io::BufWriter<std::fs::File>>,
    value: i64,
    spec: WavSpec,
) -> Result<(), hound::Error> {
    match spec.bits_per_sample {
        8 => writer.write_sample(value as i8),
        16 => writer.write_sample(value as i16),
        24 | 32 => writer.write_sample(value as i32),
        _ => Err(hound::Error::Unsupported),
    }
}
