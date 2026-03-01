use anyhow::{Context, Result};
use std::collections::HashMap;
use wasmedge_sdk::vm::SyncInst;
use wasmedge_sdk::wasi::WasiModule;
use wasmedge_sdk::{params, Module, Store, Vm, WasmVal};
use wasmedge_sdk::AsInstance;

/// Run a chain of WASM modules, piping text through each module's `process()` function.
///
/// # WASM ABI Contract
///
/// Each WASM module must export:
/// - `allocate(len: i32) -> i32` — allocates `len` bytes and returns a pointer
/// - `process(ptr: i32, len: i32) -> i64` — reads UTF-8 input from `(ptr, len)`,
///   processes it, and returns a packed i64: `(result_ptr << 32) | result_len`
///
/// The host writes the input string into the module's linear memory via `allocate`,
/// calls `process`, then reads the result string from memory.
pub fn run_wasm_chain(wasm_files: &[String], input: &str) -> Result<String> {
    let mut current_text = input.to_string();

    for (i, wasm_path) in wasm_files.iter().enumerate() {
        tracing::info!(
            "Running WASM module {}/{}: {wasm_path}",
            i + 1,
            wasm_files.len()
        );
        current_text = run_single_wasm(wasm_path, &current_text)
            .with_context(|| format!("WASM module failed: {wasm_path}"))?;
        tracing::info!("Module {} output: {current_text}", i + 1);
    }

    Ok(current_text)
}

fn run_single_wasm(wasm_path: &str, input: &str) -> Result<String> {
    // Create WASI module (needed for wasm32-wasip1 targets)
    let mut wasi = WasiModule::create(None, None, None)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to create WASI module")?;

    let mut instances: HashMap<String, &mut dyn SyncInst> = HashMap::new();
    instances.insert(wasi.name().to_string(), wasi.as_mut());

    let store = Store::new(None, instances)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to create WasmEdge store")?;

    let mut vm = Vm::new(store);

    // Load and register the WASM module as the active module
    let module = Module::from_file(None, wasm_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| format!("failed to load WASM module: {wasm_path}"))?;

    vm.register_module(None, module)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to register WASM module")?;

    let input_bytes = input.as_bytes();
    let input_len = input_bytes.len() as i32;

    // Step 1: Call allocate(len) to get a pointer in the module's memory
    let alloc_results = vm
        .run_func(None, "allocate", params!(input_len))
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to call allocate()")?;

    let input_ptr = alloc_results
        .first()
        .map(|v| v.to_i32())
        .context("allocate() did not return a value")?;

    // Step 2: Write input string into the module's linear memory
    {
        let instance = vm
            .active_module_mut()
            .context("no active module instance")?;
        let mut memory = instance
            .get_memory_mut("memory")
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("module has no exported 'memory'")?;

        memory
            .set_data(input_bytes, input_ptr as u32)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("failed to write input to WASM memory")?;
    }

    // Step 3: Call process(ptr, len) → returns packed i64 (result_ptr << 32 | result_len)
    let process_results = vm
        .run_func(None, "process", params!(input_ptr, input_len))
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to call process()")?;

    let packed = process_results
        .first()
        .map(|v| v.to_i64() as u64)
        .context("process() did not return a value")?;

    let result_ptr = (packed >> 32) as u32;
    let result_len = (packed & 0xFFFF_FFFF) as u32;

    if result_len == 0 {
        return Ok(String::new());
    }

    // Step 4: Read result string from the module's memory
    let instance = vm
        .active_module()
        .context("no active module instance")?;
    let memory = instance
        .get_memory_ref("memory")
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("module has no exported 'memory'")?;

    let result_bytes = memory
        .get_data(result_ptr, result_len)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to read result from WASM memory")?;

    String::from_utf8(result_bytes).context("WASM process() returned invalid UTF-8")
}
