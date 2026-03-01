//! LLM WASM module for voice-actions.
//!
//! Build with:
//!   RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" \
//!     cargo build --target wasm32-wasip1 --release
//!
//! Sends the input text to the OpenAI chat completions API and returns
//! the assistant's response. Requires the OPENAI_API_KEY environment variable.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Pure Rust processing — calls the OpenAI API
// ---------------------------------------------------------------------------

fn process(input: &str) -> String {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");
    rt.block_on(call_openai(input))
}

async fn call_openai(input: &str) -> String {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    let request = ChatRequest {
        model: "gpt-4o-mini".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: input.to_string(),
        }],
    };

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&request)
        .send()
        .await
        .expect("failed to send request to OpenAI");

    let body = resp
        .json::<ChatResponse>()
        .await
        .expect("failed to parse OpenAI response");

    body.choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_default()
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

// ---------------------------------------------------------------------------
// WASM ABI glue (unsafe helpers for memory management)
// ---------------------------------------------------------------------------

use std::alloc::{alloc, Layout};

/// Allocate `len` bytes in WASM linear memory and return a pointer.
#[no_mangle]
pub extern "C" fn allocate(len: i32) -> i32 {
    let len = len as usize;
    if len == 0 {
        return 0;
    }
    let layout = Layout::from_size_align(len, 1).expect("invalid layout");
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        panic!("allocation failed");
    }
    ptr as i32
}

/// Read input from WASM memory, call `process()`, write output back.
/// Returns a packed i64: `(result_ptr << 32) | result_len`.
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
