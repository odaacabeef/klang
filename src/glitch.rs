use std::path::{Path, PathBuf};

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

fn parse_bpm(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if (20.0..=300.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be between 20 and 300 BPM".to_string())
    }
}

fn parse_bars(s: &str) -> Result<u32, String> {
    let v: u32 = s
        .parse()
        .map_err(|_| "must be a whole number".to_string())?;
    if (1..=256).contains(&v) {
        Ok(v)
    } else {
        Err("must be between 1 and 256".to_string())
    }
}

fn parse_resolution(s: &str) -> Result<u32, String> {
    let v: u32 = s
        .parse()
        .map_err(|_| "must be a whole number".to_string())?;
    if [1, 2, 4, 8, 16, 32, 64].contains(&v) {
        Ok(v)
    } else {
        Err("must be one of 1, 2, 4, 8, 16, 32, 64".to_string())
    }
}

fn parse_time_sig(s: &str) -> Result<(u32, u32), String> {
    let (a, b) = s
        .split_once('/')
        .ok_or_else(|| "must be in the form N/D, e.g. 4/4".to_string())?;
    let n: u32 = a
        .trim()
        .parse()
        .map_err(|_| "numerator must be a number".to_string())?;
    let d: u32 = b
        .trim()
        .parse()
        .map_err(|_| "denominator must be a number".to_string())?;
    if !(1..=32).contains(&n) {
        return Err("numerator must be between 1 and 32".to_string());
    }
    if ![1, 2, 4, 8, 16, 32].contains(&d) {
        return Err("denominator must be 1, 2, 4, 8, 16, or 32".to_string());
    }
    Ok((n, d))
}

fn parse_unit(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be between 0.0 and 1.0".to_string())
    }
}

fn parse_gate(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| "must be a number".to_string())?;
    if v > 0.0 && v <= 1.0 {
        Ok(v)
    } else {
        Err("must be greater than 0.0 and at most 1.0".to_string())
    }
}

#[derive(clap::Args)]
pub struct Args {
    /// Input WAV files (one or more)
    #[arg(required = true, num_args = 1..)]
    inputs: Vec<PathBuf>,

    /// Output WAV file
    #[arg(short, long)]
    output: PathBuf,

    /// Tempo in BPM (20–300)
    #[arg(long, default_value_t = 120.0, value_parser = parse_bpm)]
    bpm: f32,

    /// Length in bars (1–256)
    #[arg(long, default_value_t = 4, value_parser = parse_bars)]
    bars: u32,

    /// Time signature, e.g. 4/4 or 6/8
    #[arg(long, default_value = "4/4", value_parser = parse_time_sig)]
    time_sig: (u32, u32),

    /// Grid resolution as a note division: 4=quarter, 8=eighth, 16=sixteenth (1–64)
    #[arg(long, default_value_t = 16, value_parser = parse_resolution)]
    resolution: u32,

    /// Probability that each step is filled, 0.0–1.0 (lower = sparser)
    #[arg(long, default_value_t = 0.5, value_parser = parse_unit)]
    density: f32,

    /// Swing amount, 0.0–1.0 (delays off-beat steps for groove)
    #[arg(long, default_value_t = 0.0, value_parser = parse_unit)]
    swing: f32,

    /// Slice length as a fraction of one step, 0.0–1.0 (lower = choppier)
    #[arg(long, default_value_t = 1.0, value_parser = parse_gate)]
    gate: f32,

    /// Onset detection sensitivity, 0.0–1.0 (higher = more slices)
    #[arg(long, default_value_t = 0.5, value_parser = parse_unit)]
    sensitivity: f32,

    /// Repeat a single bar's pattern instead of re-rolling each bar
    #[arg(long)]
    repeat: bool,

    /// RNG seed for reproducible output (omit for a random seed)
    #[arg(long)]
    seed: Option<u64>,
}

/// Deterministic splitmix64 PRNG — keeps output reproducible from a seed
/// without pulling in an external crate.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform float in [0.0, 1.0).
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let seed = args.seed.unwrap_or_else(|| {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    });

    // Decode every input, conform to the first file's channel layout, and
    // collect a pool of interesting slices via onset detection.
    let mut out_spec: Option<WavSpec> = None;
    let mut pool: Vec<Vec<f32>> = Vec::new();

    for path in &args.inputs {
        let (samples, spec) = read_samples(path)?;

        let out_spec = match out_spec {
            Some(s) => {
                if spec.sample_rate != s.sample_rate {
                    return Err(format!(
                        "sample rate mismatch: {} is {} Hz but the first input is {} Hz \
                         (all inputs must share a sample rate)",
                        path.display(),
                        spec.sample_rate,
                        s.sample_rate
                    )
                    .into());
                }
                s
            }
            None => {
                out_spec = Some(spec);
                spec
            }
        };

        let channels = out_spec.channels as usize;
        let conformed = conform_channels(&samples, spec.channels as usize, channels);
        let slices = slice_input(
            &conformed,
            channels,
            out_spec.sample_rate as f32,
            args.sensitivity,
        );
        pool.extend(slices);
    }

    let spec = out_spec.expect("at least one input is required");
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate as f32;

    if pool.is_empty() {
        return Err("no usable audio found in the inputs".into());
    }

    // Build the grid from tempo + time signature + resolution.
    let (num, den) = args.time_sig;
    if !(num * args.resolution).is_multiple_of(den) {
        return Err(format!(
            "a {}/{} bar does not divide evenly into 1/{} steps; try a finer --resolution",
            num, den, args.resolution
        )
        .into());
    }
    let steps_per_bar = (num * args.resolution / den) as usize;
    let total_steps = steps_per_bar * args.bars as usize;
    let step_dur_secs = 240.0 / (args.resolution as f32 * args.bpm);
    let total_frames = (total_steps as f32 * step_dur_secs * sample_rate).round() as usize;

    // Roll the pattern: which steps fire, and which slice each one plays.
    let mut rng = Rng::new(seed);
    let pattern_len = if args.repeat {
        steps_per_bar
    } else {
        total_steps
    };
    let mut pattern: Vec<Option<usize>> = Vec::with_capacity(pattern_len);
    for _ in 0..pattern_len {
        if rng.next_f32() < args.density {
            let idx = (rng.next_u64() % pool.len() as u64) as usize;
            pattern.push(Some(idx));
        } else {
            pattern.push(None);
        }
    }

    // Render: place each fired slice at its (swung) step time.
    let mut out = vec![0f32; total_frames * channels];
    let fade_frames = ((sample_rate * 0.003) as usize).max(1);
    let mut hits = 0;

    for s in 0..total_steps {
        let Some(idx) = pattern[s % pattern_len] else {
            continue;
        };

        let mut start_secs = s as f32 * step_dur_secs;
        if s % 2 == 1 {
            start_secs += args.swing * step_dur_secs * 0.5;
        }
        let start_frame = (start_secs * sample_rate).round() as usize;
        if start_frame >= total_frames {
            continue;
        }
        hits += 1;

        let gate_frames = ((args.gate * step_dur_secs * sample_rate).round() as usize).max(1);
        let slice = &pool[idx];
        let slice_frames = slice.len() / channels;
        let play = gate_frames
            .min(slice_frames)
            .min(total_frames - start_frame);
        let fade = fade_frames.min(play / 2).max(1);

        for f in 0..play {
            let env = if f < fade {
                f as f32 / fade as f32
            } else if f >= play - fade {
                (play - f) as f32 / fade as f32
            } else {
                1.0
            };
            for c in 0..channels {
                out[(start_frame + f) * channels + c] += slice[f * channels + c] * env;
            }
        }
    }

    // Normalize to a safe ceiling so overlapping hits never clip.
    let peak = out.iter().map(|s| s.abs()).fold(0f32, f32::max);
    if peak == 0.0 {
        return Err("generated audio is silent; try a higher --density or --gate".into());
    }
    let target_db = -1.0;
    let gain = 10f32.powf(target_db / 20.0) / peak;
    for s in out.iter_mut() {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }

    let tmp_path = args.output.with_extension("klang_tmp.wav");
    {
        let mut writer = WavWriter::create(&tmp_path, spec)?;
        match spec.sample_format {
            SampleFormat::Float => {
                for s in &out {
                    writer.write_sample(*s)?;
                }
            }
            SampleFormat::Int => {
                let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
                for s in &out {
                    let scaled = (s * max_val).clamp(-max_val, max_val - 1.0);
                    write_int_sample(&mut writer, scaled as i64, spec)?;
                }
            }
        }
        writer.finalize()?;
    }
    std::fs::rename(&tmp_path, &args.output)?;

    let total_secs = total_frames as f32 / sample_rate;
    println!("Glitched:");
    println!(
        "  Inputs:      {} file(s), {} slices",
        args.inputs.len(),
        pool.len()
    );
    println!(
        "  Tempo:       {} BPM, {}/{}, 1/{} grid",
        args.bpm, num, den, args.resolution
    );
    println!(
        "  Length:      {} bars ({:.2}s), {} steps",
        args.bars, total_secs, total_steps
    );
    println!(
        "  Hits:        {} / {} (density {:.2}{})",
        hits,
        total_steps,
        args.density,
        if args.repeat { ", repeating" } else { "" }
    );
    println!("  Seed:        {}", seed);

    Ok(())
}

/// Read a WAV file and decode its samples to interleaved f32 in [-1.0, 1.0].
fn read_samples(path: &Path) -> Result<(Vec<f32>, WavSpec), Box<dyn std::error::Error>> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
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
    Ok((samples, spec))
}

/// Remap interleaved samples to a different channel count by collapsing to
/// mono and spreading across the destination channels.
fn conform_channels(samples: &[f32], src_ch: usize, dst_ch: usize) -> Vec<f32> {
    if src_ch == dst_ch {
        return samples.to_vec();
    }
    let frames = samples.len() / src_ch;
    let mut out = vec![0f32; frames * dst_ch];
    for f in 0..frames {
        let mut mono = 0f32;
        for c in 0..src_ch {
            mono += samples[f * src_ch + c];
        }
        mono /= src_ch as f32;
        for c in 0..dst_ch {
            out[f * dst_ch + c] = mono;
        }
    }
    out
}

/// Find onsets via an energy-novelty detection function and cut a slice from
/// each onset to the next. Returns interleaved-f32 slices.
fn slice_input(
    samples: &[f32],
    channels: usize,
    sample_rate: f32,
    sensitivity: f32,
) -> Vec<Vec<f32>> {
    let frames = samples.len() / channels;
    if frames == 0 {
        return Vec::new();
    }

    // Per-frame peak amplitude across channels.
    let mut mono = vec![0f32; frames];
    for f in 0..frames {
        let mut m = 0f32;
        for c in 0..channels {
            m = m.max(samples[f * channels + c].abs());
        }
        mono[f] = m;
    }

    let block = ((sample_rate * 0.01) as usize).max(1); // ~10ms analysis blocks
    let nblocks = frames / block;

    let onsets = if nblocks < 3 {
        vec![0]
    } else {
        // Block RMS energy, then a half-wave-rectified first difference.
        let mut energy = vec![0f32; nblocks];
        for i in 0..nblocks {
            let mut sum = 0f32;
            for j in 0..block {
                let v = mono[i * block + j];
                sum += v * v;
            }
            energy[i] = (sum / block as f32).sqrt();
        }
        let mut df = vec![0f32; nblocks];
        for i in 1..nblocks {
            df[i] = (energy[i] - energy[i - 1]).max(0.0);
        }

        // Adaptive threshold: mean rises toward mean+3σ as sensitivity drops.
        let n = (nblocks - 1) as f32;
        let mean = df[1..].iter().sum::<f32>() / n;
        let var = df[1..].iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n;
        let threshold = mean + (1.0 - sensitivity) * 3.0 * var.sqrt();

        let min_gap = (sample_rate * 0.04) as usize; // ignore onsets <40ms apart
        let mut onsets = Vec::new();
        let mut last: Option<usize> = None;
        for i in 1..nblocks - 1 {
            if df[i] > threshold && df[i] >= df[i - 1] && df[i] > df[i + 1] {
                let frame = i * block;
                if last.is_none_or(|l| frame - l >= min_gap) {
                    onsets.push(frame);
                    last = Some(frame);
                }
            }
        }
        if onsets.is_empty() {
            onsets.push(0);
        }
        onsets
    };

    let max_slice = (sample_rate * 2.0) as usize; // cap slice length at 2s
    let mut slices = Vec::with_capacity(onsets.len());
    for k in 0..onsets.len() {
        let start = onsets[k];
        let end = onsets.get(k + 1).copied().unwrap_or(frames);
        let end = end.min(start + max_slice).min(frames);
        if end > start {
            slices.push(samples[start * channels..end * channels].to_vec());
        }
    }
    slices
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
