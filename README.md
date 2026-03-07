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
