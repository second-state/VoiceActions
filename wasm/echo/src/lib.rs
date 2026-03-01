//! Echo WASM module for voice-actions.
//!
//! Build with:
//!   cargo build --target wasm32-wasip1 --release
//!
//! This module passes the input text through unchanged.
//! It demonstrates the ABI contract expected by voice-actions:
//! - `allocate(len: i32) -> i32` — allocate a buffer in WASM linear memory
//! - `process(ptr: i32, len: i32) -> i64` — process input text, return packed (ptr, len)

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

/// Echo the input text back unchanged.
///
/// Reads `len` bytes of UTF-8 text from `ptr`, allocates a copy,
/// and returns a packed i64: `(result_ptr << 32) | result_len`.
#[no_mangle]
pub extern "C" fn process(ptr: i32, len: i32) -> i64 {
    // Read input bytes from memory
    let input_bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };

    // Allocate space for the output and copy the input unchanged
    let output_ptr = allocate(len);
    unsafe {
        std::ptr::copy_nonoverlapping(input_bytes.as_ptr(), output_ptr as *mut u8, len as usize);
    }

    // Pack pointer and length into a single i64
    ((output_ptr as i64) << 32) | (len as i64)
}
