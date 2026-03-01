use anyhow::{Context, Result};
use std::collections::HashMap;
use wasmedge_sdk::vm::SyncInst;
use wasmedge_sdk::wasi::WasiModule;
use wasmedge_sdk::{params, Module, Store, Vm};

/// Run a chain of WASM modules, piping text through each module's processing.
///
/// # WASM ABI Contract
///
/// Each WASM module is a WASI binary that:
/// - Reads input from `input.txt` in the pre-opened working directory
/// - Writes output to `output.txt` in the pre-opened working directory
///
/// The host sets up a temporary directory with `input.txt`, pre-opens it
/// for the module, calls `_start`, then reads back `output.txt`.
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
    // Create a temporary directory for WASI file I/O
    let tmp_dir = std::env::temp_dir().join(format!("voice-actions-wasm-{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir).context("failed to create temp directory")?;

    // Write input to input.txt
    let input_path = tmp_dir.join("input.txt");
    std::fs::write(&input_path, input).context("failed to write input.txt")?;

    // Pre-open the temp directory as the WASI working directory
    let preopen = format!(".:{}", tmp_dir.display());

    // Create WASI module with the pre-opened directory
    let mut wasi = WasiModule::create(Some(vec!["process"]), None, Some(vec![&preopen]))
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to create WASI module")?;

    let mut instances: HashMap<String, &mut dyn SyncInst> = HashMap::new();
    instances.insert(wasi.name().to_string(), wasi.as_mut());

    let store = Store::new(None, instances)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to create WasmEdge store")?;

    let mut vm = Vm::new(store);

    // Load and register the WASM module
    let module = Module::from_file(None, wasm_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| format!("failed to load WASM module: {wasm_path}"))?;

    vm.register_module(None, module)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to register WASM module")?;

    // Run the WASI binary's _start entrypoint
    vm.run_func(None, "_start", params!())
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("WASM _start failed")?;

    // Read output from output.txt
    let output_path = tmp_dir.join("output.txt");
    let output =
        std::fs::read_to_string(&output_path).context("failed to read output.txt from WASM")?;

    // Clean up temp directory
    let _ = std::fs::remove_dir_all(&tmp_dir);

    Ok(output)
}
