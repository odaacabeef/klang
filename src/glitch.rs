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

fn parse_max_length(s: &str) -> Result<u32, String> {
    let v: u32 = s
        .parse()
        .map_err(|_| "must be a whole number".to_string())?;
    if (1..=64).contains(&v) {
        Ok(v)
    } else {
        Err("must be between 1 and 64".to_string())
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

    /// Reference WAV whose onset timing and amplitude shape the output. When
    /// set, only --sensitivity and --seed apply; the grid options (bpm, bars,
    /// time-sig, resolution, density, swing, max-length, gate, repeat) are
    /// ignored.
    #[arg(long)]
    template: Option<PathBuf>,

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

    /// Maximum slice length in grid steps; each hit is a random 1..N (1–64)
    #[arg(long, default_value_t = 4, value_parser = parse_max_length)]
    max_length: u32,

    /// Fraction of each slice's length that sounds, 0.0–1.0 (1.0 = exact grid
    /// multiple; lower = choppier)
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

    let mut rng = Rng::new(seed);
    let fade_frames = ((sample_rate * 0.003) as usize).max(1);

    // Build the output buffer from either a reference template or the grid.
    let (mut out, report): (Vec<f32>, Vec<String>) = if let Some(template_path) = &args.template {
        // Template mode: follow the template's onset timing and amplitude,
        // rebuilding it from the input slices. Grid options are ignored.
        let (t_samples, t_spec) = read_samples(template_path)?;
        if t_spec.sample_rate != spec.sample_rate {
            return Err(format!(
                "sample rate mismatch: template {} is {} Hz but the inputs are {} Hz",
                template_path.display(),
                t_spec.sample_rate,
                spec.sample_rate
            )
            .into());
        }
        let template = conform_channels(&t_samples, t_spec.channels as usize, channels);
        let template_frames = template.len() / channels;
        if template_frames == 0 {
            return Err("template file is empty".into());
        }
        let onsets = detect_onsets(&template, channels, sample_rate, args.sensitivity);

        let mut out = vec![0f32; template_frames * channels];
        let mut placed = 0;
        for k in 0..onsets.len() {
            let start_frame = onsets[k];
            let end_frame = onsets.get(k + 1).copied().unwrap_or(template_frames);

            // The template's peak over this window sets how loud the slice plays.
            let mut target_amp = 0f32;
            for f in start_frame..end_frame {
                for c in 0..channels {
                    target_amp = target_amp.max(template[f * channels + c].abs());
                }
            }
            if target_amp == 0.0 {
                continue; // a silent stretch of the template stays silent
            }

            // The slice rings until the template's next onset.
            let idx = (rng.next_u64() % pool.len() as u64) as usize;
            let slice = &pool[idx];
            let slice_frames = slice.len() / channels;
            let play = (end_frame - start_frame).min(slice_frames);
            if play == 0 {
                continue;
            }

            // Scale the slice so its peak matches the template's local amplitude.
            let mut slice_peak = 0f32;
            for f in 0..play {
                for c in 0..channels {
                    slice_peak = slice_peak.max(slice[f * channels + c].abs());
                }
            }
            if slice_peak == 0.0 {
                continue;
            }
            let gain = target_amp / slice_peak;

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
                    out[(start_frame + f) * channels + c] += slice[f * channels + c] * gain * env;
                }
            }
            placed += 1;
        }

        let report = vec![
            format!(
                "  Template:    {} ({:.2}s, {} events)",
                template_path.display(),
                template_frames as f32 / sample_rate,
                onsets.len()
            ),
            format!("  Placed:      {} slices (timing + amplitude)", placed),
        ];
        (out, report)
    } else {
        // Grid mode: a tempo/time-signature grid fired by --density.
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

        // Roll the pattern: which steps fire, the slice each plays, and how many
        // grid steps long it is (a random 1..=max_length).
        let pattern_len = if args.repeat {
            steps_per_bar
        } else {
            total_steps
        };
        let mut pattern: Vec<Option<(usize, usize)>> = Vec::with_capacity(pattern_len);
        for _ in 0..pattern_len {
            if rng.next_f32() < args.density {
                let idx = (rng.next_u64() % pool.len() as u64) as usize;
                let length_steps = (rng.next_u64() % args.max_length as u64) as usize + 1;
                pattern.push(Some((idx, length_steps)));
            } else {
                pattern.push(None);
            }
        }

        // Render: place each fired slice at its (swung) step time.
        let mut out = vec![0f32; total_frames * channels];
        let mut hits = 0;
        for s in 0..total_steps {
            let Some((idx, length_steps)) = pattern[s % pattern_len] else {
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

            // The hit spans `length_steps` grid steps; `gate` trims how much sounds.
            let slot_secs = length_steps as f32 * step_dur_secs * args.gate;
            let slot_frames = ((slot_secs * sample_rate).round() as usize).max(1);
            let slice = &pool[idx];
            let slice_frames = slice.len() / channels;
            let play = slot_frames
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

        let total_secs = total_frames as f32 / sample_rate;
        let report = vec![
            format!(
                "  Tempo:       {} BPM, {}/{}, 1/{} grid",
                args.bpm, num, den, args.resolution
            ),
            format!(
                "  Length:      {} bars ({:.2}s), {} steps",
                args.bars, total_secs, total_steps
            ),
            format!(
                "  Hits:        {} / {} (density {:.2}{})",
                hits,
                total_steps,
                args.density,
                if args.repeat { ", repeating" } else { "" }
            ),
            format!(
                "  Lengths:     1–{} steps (gate {:.2})",
                args.max_length, args.gate
            ),
        ];
        (out, report)
    };

    // Normalize to a safe ceiling so overlapping hits never clip.
    let peak = out.iter().map(|s| s.abs()).fold(0f32, f32::max);
    if peak == 0.0 {
        return Err("generated audio is silent".into());
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

    println!("Glitched:");
    println!(
        "  Inputs:      {} file(s), {} slices",
        args.inputs.len(),
        pool.len()
    );
    for line in &report {
        println!("{line}");
    }
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

/// Find onsets via an energy-novelty detection function: block RMS energy, a
/// half-wave-rectified first difference, an adaptive threshold, and peak
/// picking. Returns the frame index of each onset.
fn detect_onsets(
    samples: &[f32],
    channels: usize,
    sample_rate: f32,
    sensitivity: f32,
) -> Vec<usize> {
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
    if nblocks < 3 {
        return vec![0];
    }

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
}

/// Cut a slice from each detected onset to the next. Returns interleaved-f32
/// slices.
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
    let onsets = detect_onsets(samples, channels, sample_rate, sensitivity);

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
