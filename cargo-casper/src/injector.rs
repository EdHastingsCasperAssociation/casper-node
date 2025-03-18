use anyhow::Context;
use include_dir::{include_dir, Dir};
use std::path::PathBuf;
use tempfile::TempDir;

use crate::cli::extract_embedded_dir;
use crate::compilation::CompileJob;

// Embed the entire "injected" directory into the binary.
static INJECTED_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/schema-inject");

const INJECT_SCHEMA_MARKER: &str = "{{__CARGO_CASPER_INJECT_SCHEMA_MARKER}}";

/// Builds the specified WASM, while also injecting the specified schema
/// into it.
pub fn build_with_schema_injected(
    user_lib_compilation: CompileJob,
    schema: &str,
    production_wasm_output_dir: &PathBuf,
) -> Result<PathBuf, anyhow::Error> {
    // Construct a temp directory for compilation
    let temp_dir = TempDir::new()
        .with_context(|| "Failed creating temporary directory")?;

    // Compile the schema-inject crate, with the appropriate schema string
    // injected into it.
    let schema_crate_path = extract_embedded_dir(
        &temp_dir.path().join("schema-inject"),
        &INJECTED_DIR,
    ).with_context(|| "Failed extracting the schema-inject crate")?;

    let schema_lib_path = schema_crate_path
        .join("src/lib.rs");

    let mut schema_lib_contents = std::fs::read_to_string(&schema_lib_path)
        .with_context(|| "Failed reading schema-inject's lib.rs")?;

    schema_lib_contents = schema_lib_contents.replace(
        INJECT_SCHEMA_MARKER,
        schema
    );

    std::fs::write(&schema_lib_path, schema_lib_contents)
        .with_context(|| "Failed writing to schema-inject's lib.rs")?;

    let schema_manifest = schema_crate_path.join("Cargo.toml");
    let schema_compilation = CompileJob::new(
        schema_manifest.to_str().unwrap(),
        None,
        None
    );

    let schema_compilation_results = schema_compilation.dispatch(
        "wasm32-unknown-unknown",
        Option::<String>::None
    ).with_context(|| "Failed compiling the schema-inject crate")?;

    let schema_lib = schema_compilation_results
        .artifacts()
        .iter()
        .find(|x| x.extension().unwrap_or_default() == "a")
        .with_context(|| "Couldn't find the compiled schema-inject lib")?;

    // Build user's wasm with the schema lib statically linked and exported
    let rustflags = format!(
        "-C link-arg={} -C link-arg=--export=__casper_schema",
        schema_lib.to_string_lossy()
    );

    let production_wasm_path = user_lib_compilation
        .with_rustflags(rustflags)
        .dispatch("wasm32-unknown-unknown", Option::<String>::None)
        .context("Failed to compile user wasm")?
        .flush_artifacts_to_dir(production_wasm_output_dir)
        .context("Failed extracting build artifacts to directory")?;

    Ok(production_wasm_path)
}