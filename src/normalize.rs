use std::path::PathBuf;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

fn parse_target_db(s: &str) -> Result<f32, String> {
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

    /// Output WAV file (defaults to overwriting input)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Target peak level in dBFS (-20 to 0)
    #[arg(short, long, default_value_t = 0.0, value_parser = parse_target_db)]
    target_db: f32,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let output_path = args.output.unwrap_or_else(|| args.input.clone());

    let mut reader = WavReader::open(&args.input)?;
    let spec = reader.spec();

    let target_linear = 10f32.powf(args.target_db / 20.0);

    // Read all samples as f32 for processing
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

    let peak = samples.iter().map(|s| s.abs()).fold(0f32, f32::max);

    if peak == 0.0 {
        return Err("Audio is silent; cannot normalize".into());
    }

    let gain = target_linear / peak;

    // Write to a temp file first, then rename (handles in-place case safely)
    let tmp_path = output_path.with_extension("klang_tmp.wav");
    {
        let mut writer = WavWriter::create(&tmp_path, spec)?;
        match spec.sample_format {
            SampleFormat::Float => {
                for s in &samples {
                    writer.write_sample((s * gain).clamp(-1.0, 1.0))?;
                }
            }
            SampleFormat::Int => {
                let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
                for s in &samples {
                    let scaled = (s * gain * max_val).clamp(-max_val, max_val - 1.0);
                    write_int_sample(&mut writer, scaled as i64, spec)?;
                }
            }
        }
        writer.finalize()?;
    }

    std::fs::rename(&tmp_path, &output_path)?;

    println!(
        "Normalized: peak {:.2} dBFS → {:.2} dBFS (gain {:.2} dB)",
        20.0 * peak.log10(),
        args.target_db,
        20.0 * gain.log10()
    );

    Ok(())
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
