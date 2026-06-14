# klang

A CLI toolkit for manipulating WAV files.

## Installation

```sh
cargo install --path .
```

## Commands

### `info`

Print metadata about a WAV file.

```sh
klang info <INPUT>
```

**Example:**

```
$ klang info recording.wav
File:        recording.wav
Format:      PCM int 16-bit
Sample rate: 44100 Hz
Channels:    2
Duration:    03:27.500
Samples:     18249750
Peak:        -1.24 dBFS
File size:   34.93 MB
```

---

### `normalize`

Normalize audio to a target peak level.

```sh
klang normalize [OPTIONS] <INPUT>
```

**Options:**

| Flag | Description | Range | Default |
|------|-------------|-------|---------|
| `-o, --output <FILE>` | Output file (omit to overwrite input) | | in-place |
| `-t, --target-db <DB>` | Target peak level in dBFS | -20 to 0 dBFS | `0.0` |

**Examples:**

```sh
# Normalize in place to 0 dBFS
klang normalize recording.wav

# Write to a new file
klang normalize recording.wav -o recording_normalized.wav

# Normalize to -3 dBFS
klang normalize recording.wav -t -3.0
```

---

### `master`

Apply a mastering chain to a WAV file: high-pass filter, compression, limiting, and normalization.

```sh
klang master [OPTIONS] <INPUT>
```

**Options:**

| Flag | Description | Range | Default |
|------|-------------|-------|---------|
| `-o, --output <FILE>` | Output file (omit to overwrite input) | | in-place |
| `--highpass-hz <HZ>` | High-pass filter cutoff | 0–20000 Hz | `30.0` |
| `--comp-threshold-db <DB>` | Compressor threshold | -60 to -1 dBFS | `-18.0` |
| `--comp-ratio <RATIO>` | Compressor ratio (1.0 = no compression) | 1.0–20.0 | `3.0` |
| `--comp-attack-ms <MS>` | Compressor attack time | 0.1–500 ms | `10.0` |
| `--comp-release-ms <MS>` | Compressor release time | 1–2000 ms | `100.0` |
| `--limiter-ceiling-db <DB>` | Limiter ceiling | -20 to 0 dBFS | `-1.0` |
| `-t, --target-db <DB>` | Output peak target | -20 to 0 dBFS | `-1.0` |

**Examples:**

```sh
# Master in place with defaults
klang master recording.wav

# Write to a new file
klang master recording.wav -o recording_mastered.wav

# Custom compressor settings
klang master recording.wav --comp-threshold-db -12 --comp-ratio 4
```

**Example output:**

```
$ klang master recording.wav -o recording_mastered.wav
Mastered:
  High-pass:   30 Hz
  Compression: threshold -18.0 dBFS, ratio 3.0:1, attack 10ms, release 100ms
  Limiter:     ceiling -1.0 dBFS
  Normalized:  peak -4.32 dBFS → -1.00 dBFS
```

---

### `glitch`

Slice interesting moments from one or more input WAV files and arrange them on
a tempo-driven grid to produce a rhythmic, glitchy mashup.

The command detects onsets (transients) in each input, builds a pool of slices,
then walks a grid defined by the tempo, time signature, and resolution — firing
slices according to `--density` and `--swing`. Output is normalized to -1 dBFS.
Pass a `--seed` for reproducible results.

```sh
klang glitch [OPTIONS] --output <OUTPUT> <INPUTS>...
```

**Options:**

| Flag | Description | Range | Default |
|------|-------------|-------|---------|
| `-o, --output <FILE>` | Output file (required) | | |
| `--bpm <BPM>` | Tempo | 20–300 | `120` |
| `--bars <N>` | Length in bars | 1–256 | `4` |
| `--time-sig <N/D>` | Time signature, e.g. `4/4`, `6/8` | N 1–32, D ∈ {1,2,4,8,16,32} | `4/4` |
| `--resolution <DIV>` | Grid resolution (note division) | 1, 2, 4, 8, 16, 32, 64 | `16` |
| `--density <P>` | Probability each step fires (lower = sparser) | 0.0–1.0 | `0.5` |
| `--swing <AMT>` | Delays off-beat steps for groove | 0.0–1.0 | `0.0` |
| `--gate <FRAC>` | Slice length as a fraction of a step (lower = choppier) | 0.0–1.0 | `1.0` |
| `--sensitivity <S>` | Onset detection sensitivity (higher = more slices) | 0.0–1.0 | `0.5` |
| `--repeat` | Repeat a single bar's pattern instead of re-rolling each bar | flag | off |
| `--seed <N>` | RNG seed (omit for a random seed) | | random |

All inputs must share a sample rate; the output adopts the first input's format
and channel count (other inputs are mixed to match).

**Examples:**

```sh
# Mash two breaks into 4 bars at 120 BPM
klang glitch break1.wav break2.wav -o mashup.wav

# Sparse, swung, choppy 2-bar loop with a fixed seed
klang glitch drums.wav vocals.wav -o loop.wav \
  --bpm 140 --bars 2 --density 0.3 --swing 0.6 --gate 0.5 --seed 42

# Repeating one-bar pattern in 6/8
klang glitch perc.wav -o groove.wav --time-sig 6/8 --resolution 8 --repeat
```

**Example output:**

```
$ klang glitch break1.wav break2.wav -o mashup.wav --seed 42
Glitched:
  Inputs:      2 file(s), 27 slices
  Tempo:       120 BPM, 4/4, 1/16 grid
  Length:      4 bars (8.00s), 64 steps
  Hits:        33 / 64 (density 0.50)
  Seed:        42
```
