//! Example WASM module for voice-actions.
//!
//! Build with:
//!   cargo build --target wasm32-wasip1 --release
//!
//! This module demonstrates the ABI contract expected by voice-actions:
//! - `allocate(len: i32) -> i32` — allocate a buffer in WASM linear memory
//! - `process(ptr: i32, len: i32) -> i64` — process input text, return packed (ptr, len)
//!
//! This example simply converts the input text to uppercase.

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

/// Process the input text and return a result.
///
/// Reads `len` bytes of UTF-8 text from `ptr`, transforms it,
/// and returns a packed i64: `(result_ptr << 32) | result_len`.
#[no_mangle]
pub extern "C" fn process(ptr: i32, len: i32) -> i64 {
    // Read input string from memory
    let input = unsafe {
        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        String::from_utf8_lossy(slice).into_owned()
    };

    // --- Your processing logic here ---
    // This example converts to uppercase. In a real module, you could:
    // - Call an HTTP API
    // - Parse and transform the text
    // - Look up data
    // - Chain with LLM APIs
    let output = input.to_uppercase();

    // Allocate space for the result and copy it
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

    // Pack pointer and length into a single i64
    ((output_ptr as i64) << 32) | (output_len as i64)
}
