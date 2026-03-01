//! Echo WASM module for voice-actions.
//!
//! Build with:
//!   cargo build --target wasm32-wasip1 --release

// ---------------------------------------------------------------------------
// Pure Rust processing — this is the only function you need to change
// ---------------------------------------------------------------------------

fn process(input: &str) -> String {
    input.to_string()
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
