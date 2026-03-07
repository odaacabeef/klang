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

| Flag | Description | Default |
|------|-------------|---------|
| `-o, --output <FILE>` | Output file (omit to overwrite input) | in-place |
| `-t, --target-db <DB>` | Target peak level in dBFS | `0.0` |

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

| Flag | Description | Default |
|------|-------------|---------|
| `-o, --output <FILE>` | Output file (omit to overwrite input) | in-place |
| `--highpass-hz <HZ>` | High-pass filter cutoff | `30.0` |
| `--comp-threshold-db <DB>` | Compressor threshold | `-18.0` |
| `--comp-ratio <RATIO>` | Compressor ratio | `3.0` |
| `--comp-attack-ms <MS>` | Compressor attack time | `10.0` |
| `--comp-release-ms <MS>` | Compressor release time | `100.0` |
| `--limiter-ceiling-db <DB>` | Limiter ceiling | `-1.0` |
| `-t, --target-db <DB>` | Output peak target | `-1.0` |

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
