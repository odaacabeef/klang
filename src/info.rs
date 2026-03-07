use std::path::PathBuf;

use hound::{SampleFormat, WavReader};

#[derive(clap::Args)]
pub struct Args {
    /// Input WAV file
    input: PathBuf,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = WavReader::open(&args.input)?;
    let spec = reader.spec();

    let num_samples = reader.len();
    let num_channels = spec.channels as u64;
    let sample_rate = spec.sample_rate as u64;
    let duration_secs = if num_channels > 0 && sample_rate > 0 {
        num_samples as f64 / (num_channels as f64 * sample_rate as f64)
    } else {
        0.0
    };

    let format = match spec.sample_format {
        SampleFormat::Float => "float".to_string(),
        SampleFormat::Int => "int".to_string(),
    };

    let peak = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|s| s.abs())
            .fold(0f32, f32::max),
        SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|s| s as f32 / max_val)
                .fold(0f32, |a, s| a.max(s.abs()))
        }
    };

    let peak_db = if peak > 0.0 {
        format!("{:.2} dBFS", 20.0 * peak.log10())
    } else {
        "-inf dBFS".to_string()
    };

    let file_size = std::fs::metadata(&args.input)?.len();

    let minutes = (duration_secs / 60.0) as u64;
    let seconds = duration_secs % 60.0;

    println!("File:        {}", args.input.display());
    println!("Format:      PCM {} {}-bit", format, spec.bits_per_sample);
    println!("Sample rate: {} Hz", spec.sample_rate);
    println!("Channels:    {}", spec.channels);
    println!("Duration:    {:02}:{:06.3}", minutes, seconds);
    println!("Samples:     {}", num_samples);
    println!("Peak:        {}", peak_db);
    println!("File size:   {}", format_bytes(file_size));

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    match bytes {
        b if b >= GB => format!("{:.2} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.2} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.2} KB", b as f64 / KB as f64),
        b => format!("{} B", b),
    }
}
