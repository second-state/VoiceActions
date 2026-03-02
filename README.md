# voice-actions

A voice-to-voice processing pipeline that transcribes audio to text, runs the text through a chain of WebAssembly processing modules, and synthesizes the result back to speech.

```
Audio Input → ASR (Qwen3) → WASM chain → TTS (Qwen3) → MP3 Output
```

## Features

- **Speech-to-text** using [Qwen3-ASR](https://github.com/second-state/qwen3_asr_rs) — accepts any audio format (WAV, MP3, FLAC, Opus, etc.)
- **Text-to-speech** using [Qwen3-TTS](https://github.com/second-state/qwen3_tts_rs) — custom voice and speaker support
- **WASM processing chain** — plug in one or more WebAssembly modules to transform text between ASR and TTS (call APIs, run LLMs, look up data, etc.)
- **Multi-platform** — Linux (CPU/CUDA via libtorch) and macOS Apple Silicon (Metal GPU via MLX)
- **Self-contained builds** — FFmpeg built from source, WasmEdge and codec libraries bundled

## Quick Start

Download the latest release archive for your platform from
[GitHub Releases](https://github.com/second-state/VoiceActions/releases)
and extract it:

```bash
# Linux
tar xzf voice-actions-linux-x86_64.tar.gz
cd voice-actions-linux-x86_64

# macOS
# unzip voice-actions-macos-aarch64.zip && cd voice-actions-macos-aarch64
```

Download the required models and generate `tokenizer.json`:

```bash
pip install huggingface_hub transformers

huggingface-cli download Qwen/Qwen3-ASR-0.6B --local-dir models/Qwen3-ASR-0.6B
huggingface-cli download Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice --local-dir models/Qwen3-TTS-12Hz-0.6B-CustomVoice

python3 -c "
from transformers import AutoTokenizer
for model in ['Qwen3-ASR-0.6B', 'Qwen3-TTS-12Hz-0.6B-CustomVoice']:
    path = f'models/{model}'
    tok = AutoTokenizer.from_pretrained(path, trust_remote_code=True)
    tok.backend_tokenizer.save(f'{path}/tokenizer.json')
"
```

Run with the bundled `echo.wasm` module (passes text through unchanged):

```bash
./voice-actions \
  --input recording.mp3 \
  --output response.mp3 \
  --asr-model ./models/Qwen3-ASR-0.6B \
  --tts-model ./models/Qwen3-TTS-12Hz-0.6B-CustomVoice \
  --wasm echo.wasm
```

All required shared libraries (WasmEdge, libopus, libmp3lame) are bundled in the
release archive — no additional installation needed.

## CLI Reference

```
voice-actions [OPTIONS] --input <PATH> --output <PATH> --asr-model <PATH> --tts-model <PATH> --wasm <PATH>...

Options:
  -i, --input <PATH>       Input audio file (any format: wav, mp3, flac, opus, etc.)
  -o, --output <PATH>      Output MP3 file
      --asr-model <PATH>   Path to Qwen3-ASR model directory
      --tts-model <PATH>   Path to Qwen3-TTS model directory
      --wasm <PATH>...     WASM module(s) to chain — executed in order
      --language <LANG>    Language hint for ASR (e.g. "en", "zh") — auto-detected if omitted
      --speaker <NAME>     TTS speaker name [default: Vivian]
  -h, --help               Print help
  -V, --version            Print version
```

Multiple `--wasm` flags chain modules sequentially — the output of each module becomes the input to the next:

```bash
voice-actions \
  --input question.wav \
  --output answer.mp3 \
  --asr-model ./models/Qwen3-ASR-0.6B \
  --tts-model ./models/Qwen3-TTS-12Hz-0.6B-CustomVoice \
  --wasm translate.wasm \
  --wasm summarize.wasm \
  --wasm respond.wasm
```

## Pipeline

```
┌──────────────┐    ┌────────────────────────────────┐    ┌──────────────┐    ┌──────────┐
│  Input Audio │───▶│  Qwen3-ASR (speech-to-text)    │───▶│  WASM Chain  │───▶│ Qwen3-TTS│───▶ MP3
│  (any format)│    │  auto-detects language          │    │  process()   │    │ (24kHz)  │
└──────────────┘    └────────────────────────────────┘    │  process()   │    └──────────┘
                                                          │  ...         │
                                                          └──────────────┘
```

1. **ASR** — Qwen3-ASR transcribes the input audio to text. Handles any FFmpeg-compatible format internally.
2. **WASM chain** — Text is piped through each WASM module's `process()` function sequentially.
3. **TTS** — Qwen3-TTS synthesizes the final text to 24 kHz raw audio samples using the selected speaker voice.
4. **Encode** — Raw audio samples are encoded to a 192 kbps MP3 via embedded FFmpeg (libmp3lame).

## Models

Download models from Hugging Face and generate the required `tokenizer.json`:

```bash
# Download models
huggingface-cli download Qwen/Qwen3-ASR-0.6B --local-dir models/Qwen3-ASR-0.6B
huggingface-cli download Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice --local-dir models/Qwen3-TTS-12Hz-0.6B-CustomVoice

# Generate tokenizer.json for each model
python3 -c "
from transformers import AutoTokenizer
for model in ['Qwen3-ASR-0.6B', 'Qwen3-TTS-12Hz-0.6B-CustomVoice']:
    path = f'models/{model}'
    tok = AutoTokenizer.from_pretrained(path, trust_remote_code=True)
    tok.backend_tokenizer.save(f'{path}/tokenizer.json')
"
```

### Available Models

| Model | Type | Parameters | Use case |
|---|---|---|---|
| `Qwen3-ASR-0.6B` | ASR | 600M | Speech-to-text transcription |
| `Qwen3-ASR-1.7B` | ASR | 1.7B | Higher accuracy transcription |
| `Qwen3-TTS-12Hz-0.6B-CustomVoice` | TTS | 600M | Named speakers (Vivian, Ryan, etc.) |
| `Qwen3-TTS-12Hz-0.6B-Base` | TTS | 600M | Voice cloning from reference audio |
| `Qwen3-TTS-12Hz-1.7B-CustomVoice` | TTS | 1.7B | Higher quality named speakers |

### TTS Speakers

The `Qwen3-TTS-12Hz-*-CustomVoice` models support named speakers: Vivian, Serena, Ryan, Aiden, Uncle_fu, Ono_anna, Sohee, Eric, Dylan, and more.

### Language Support

ASR supports 30 languages including: English, Chinese, Cantonese, Japanese, Korean, French, German, Spanish, Portuguese, Russian, Arabic, Thai, Vietnamese, Indonesian, Italian, Turkish, Hindi, and more.

## Building from Source

### macOS Apple Silicon (MLX backend)

```bash
# Install build dependencies
brew install cmake nasm pkg-config opus lame

# Build with MLX Metal GPU acceleration and embedded FFmpeg
cargo build --release --no-default-features --features mlx,build-ffmpeg
```

No libtorch or PyTorch installation required. MLX uses the Metal GPU natively.

### Linux (libtorch backend)

```bash
# Install build dependencies
sudo apt-get install -y nasm libclang-dev pkg-config libopus-dev libmp3lame-dev

# Download libtorch — pick ONE of the following:

# Linux x86_64 (CPU)
curl -LO https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.7.1%2Bcpu.zip
unzip libtorch-cxx11-abi-shared-with-deps-2.7.1+cpu.zip

# Linux ARM64 (CPU)
curl -LO https://github.com/second-state/libtorch-releases/releases/download/v2.7.1/libtorch-cxx11-abi-aarch64-2.7.1.tar.gz
tar xzf libtorch-cxx11-abi-aarch64-2.7.1.tar.gz

# Linux x86_64 (CUDA 12.8)
curl -LO https://download.pytorch.org/libtorch/cu128/libtorch-cxx11-abi-shared-with-deps-2.7.1%2Bcu128.zip
unzip libtorch-cxx11-abi-shared-with-deps-2.7.1+cu128.zip

# Set environment and build
export LIBTORCH=$PWD/libtorch
export LIBTORCH_CXX11_ABI=1
export LD_LIBRARY_PATH=$LIBTORCH/lib:$LD_LIBRARY_PATH
cargo build --release --features build-ffmpeg
```

> **Important:** Always download libtorch directly. Do not use `pip install torch` to obtain libtorch.

### Feature Flags

| Feature | Description |
|---|---|
| `tch-backend` | **(default)** PyTorch/libtorch backend — Linux CPU/CUDA, macOS CPU |
| `mlx` | Apple MLX backend — macOS Apple Silicon with Metal GPU |
| `build-ffmpeg` | Build FFmpeg from source and link statically |
| `static-ffmpeg` | Link a pre-built static FFmpeg |

The `tch-backend` and `mlx` features are **mutually exclusive**. Enabling both is a compile error.

## Writing WASM Modules

Each WASM module must be compiled to `wasm32-wasip1` and export two functions:

### ABI Contract

```
allocate(len: i32) -> i32
```

Allocate `len` bytes in WASM linear memory. Return a pointer.

```
run(ptr: i32, len: i32) -> i64
```

Read UTF-8 input from `(ptr, len)`, process it, and return a packed i64:
`(result_ptr << 32) | result_len`.

### Example Module (Rust)

The only function you need to change is `process()` — the ABI glue below handles memory management:

```rust
// ---------------------------------------------------------------------------
// Your processing logic — this is the only function you need to change
// ---------------------------------------------------------------------------

fn process(input: &str) -> String {
    // Example: pass text through unchanged (echo)
    input.to_string()

    // Or transform it:
    // input.to_uppercase()
}

// ---------------------------------------------------------------------------
// WASM ABI glue (copy as-is into new modules)
// ---------------------------------------------------------------------------

use std::alloc::{alloc, Layout};

#[no_mangle]
pub extern "C" fn allocate(len: i32) -> i32 {
    let len = len as usize;
    if len == 0 { return 0; }
    let layout = Layout::from_size_align(len, 1).expect("invalid layout");
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() { panic!("allocation failed"); }
    ptr as i32
}

#[no_mangle]
pub extern "C" fn run(ptr: i32, len: i32) -> i64 {
    let input = unsafe {
        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        String::from_utf8_lossy(slice).into_owned()
    };

    let output = process(&input);

    let output_bytes = output.as_bytes();
    let output_len = output_bytes.len() as i32;
    let output_ptr = allocate(output_len);

    unsafe {
        std::ptr::copy_nonoverlapping(
            output_bytes.as_ptr(),
            output_ptr as *mut u8,
            output_bytes.len(),
        );
    }

    ((output_ptr as i64) << 32) | (output_len as i64)
}
```

### Building a WASM Module

```bash
# Add the target (once)
rustup target add wasm32-wasip1

# Build
cargo build --target wasm32-wasip1 --release
```

The output `.wasm` file will be at `target/wasm32-wasip1/release/<name>.wasm`.

A complete working example is in the [`wasm/echo/`](wasm/echo/) directory:

```bash
cargo build --target wasm32-wasip1 --release --manifest-path wasm/echo/Cargo.toml
```

### WASM Module Capabilities

Since modules target `wasm32-wasip1` and run on WasmEdge, they can:

- Make HTTP requests (via `wasmedge_http_req` or `wasmedge_wasi_socket`)
- Read/write files through WASI
- Perform arbitrary text transformations
- Call external LLM APIs
- Access databases

## Project Structure

```
VoiceActions/
├── Cargo.toml                          # Root manifest with feature flags
├── build.rs                            # Sets rpath for bundled shared libraries
├── src/
│   ├── main.rs                         # CLI entry point and pipeline orchestration
│   ├── asr.rs                          # Qwen3-ASR wrapper (speech-to-text)
│   ├── tts.rs                          # Qwen3-TTS wrapper (text-to-speech)
│   ├── wasm_runner.rs                  # WasmEdge WASM loading and process() calling
│   └── audio.rs                        # FFmpeg MP3 encoding (raw samples → MP3)
├── wasm/
│   ├── echo/                           # Echo module — passes text through unchanged
│   └── llm/                            # LLM module — calls OpenAI-compatible APIs
├── models/                             # Model directories (git-ignored)
│   ├── Qwen3-ASR-0.6B/
│   ├── Qwen3-TTS-12Hz-0.6B-Base/
│   └── Qwen3-TTS-12Hz-0.6B-CustomVoice/
└── .github/workflows/
    ├── ci.yml                          # CI: build + test on push/PR
    └── release.yml                     # Release: build + package + upload
```

## CI/CD

### CI (`ci.yml`)

Runs on every push to `main` and on pull requests. Four jobs:

| Job | Runner | Backend | What it does |
|---|---|---|---|
| Linux x86_64 (tch-backend) | `ubuntu-latest` | tch | Downloads libtorch CPU, builds with `build-ffmpeg`, runs tests |
| Linux ARM64 (tch-backend) | `ubuntu-24.04-arm` | tch | Downloads libtorch ARM64, builds with `build-ffmpeg`, runs tests |
| macOS ARM64 (mlx) | `macos-latest` | mlx | Builds with MLX + `build-ffmpeg`, runs tests |
| Lint & Format | `ubuntu-latest` | — | `cargo fmt --check` on all crates |

### Release (`release.yml`)

Triggered when a GitHub release is published. Builds 4 platform variants and uploads archives as release assets.

| Asset | Backend | Archive | Contents |
|---|---|---|---|
| `voice-actions-linux-x86_64.tar.gz` | tch (CPU) | tar.gz | binary, echo.wasm, libtorch/, lib/ (libwasmedge, libopus, libmp3lame) |
| `voice-actions-linux-x86_64-cuda.tar.gz` | tch (CUDA 12.8) | tar.gz | binary, echo.wasm, libtorch/ (CUDA), lib/ (libwasmedge, libopus, libmp3lame) |
| `voice-actions-linux-aarch64.tar.gz` | tch (ARM64) | tar.gz | binary, echo.wasm, libtorch/, lib/ (libwasmedge, libopus, libmp3lame) |
| `voice-actions-macos-aarch64.zip` | mlx | zip | binary, echo.wasm, mlx.metallib, lib/ (libwasmedge, libopus, libmp3lame) |

### Using a Release Archive

**Linux:**

```bash
tar xzf voice-actions-linux-x86_64.tar.gz
cd voice-actions-linux-x86_64

# Bundled libs in lib/ and libtorch/ are found via RPATH ($ORIGIN/lib, $ORIGIN/libtorch/lib).
# echo.wasm is included — use it directly or supply your own WASM modules.

./voice-actions \
  --input recording.mp3 \
  --output response.mp3 \
  --asr-model /path/to/Qwen3-ASR-0.6B \
  --tts-model /path/to/Qwen3-TTS-12Hz-0.6B-CustomVoice \
  --wasm echo.wasm
```

**macOS:**

```bash
unzip voice-actions-macos-aarch64.zip
cd voice-actions-macos-aarch64

# Bundled libs in lib/ are referenced via @executable_path/lib/ — no env vars needed.
# echo.wasm is included — use it directly or supply your own WASM modules.

./voice-actions \
  --input recording.mp3 \
  --output response.mp3 \
  --asr-model /path/to/Qwen3-ASR-0.6B \
  --tts-model /path/to/Qwen3-TTS-12Hz-0.6B-CustomVoice \
  --wasm echo.wasm
```

## Logging

Set the `RUST_LOG` environment variable to control log output:

```bash
# Show info-level logs (default pipeline progress)
RUST_LOG=info ./voice-actions ...

# Show debug-level logs (detailed internals)
RUST_LOG=debug ./voice-actions ...

# Quiet mode (errors only)
RUST_LOG=error ./voice-actions ...
```

## License

Apache-2.0
