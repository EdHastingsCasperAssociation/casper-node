use anyhow::Context;
use clap_cargo::Workspace;
use include_dir::{include_dir, Dir};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};
use tempfile::TempDir;

use crate::compilation::CompilationResults;
use crate::compilation::CompileJob;

// Embed the entire "injected" directory into the binary.
static INJECTED_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/schema-inject");

/// Writes the binary-embedded schema-inject crate into a temporary directory.
/// Returns the path to the extracted crate.
/// 
/// In the future, if there are more directories embedded within this CLI binary,
/// this method should be modified to take a [`Dir`] argument. Right now, there is
/// only one ([`INJECTED_DIR`]), so the method will extract that.
fn extract_embedded_crate(temp_dir: &Path) -> io::Result<PathBuf> {
    let path_to_write = temp_dir.join("schema-inject");
    fs::create_dir_all(&path_to_write)?;

    for file in INJECTED_DIR.files() {
        let relative_path = file.path();
        let dest_path = path_to_write.join(relative_path);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest_path, file.contents())?;
    }

    Ok(path_to_write)
}

/// Builds the specified WASM, while also injecting the specified schema
/// into it.
pub fn build_with_schema_injected(
    user_lib_compilation: CompileJob,
    schema: &str
) -> Result<CompilationResults, anyhow::Error> {
    // Construct a temp directory for compilation
    let temp_dir = TempDir::new()
        .with_context(|| "Failed creating temporary directory")?;

    // Compile the schema-inject crate, with the appropriate schema string
    // injected into it.
    let schema_crate_path = extract_embedded_crate(&temp_dir.path())
        .with_context(|| "Failed extracting the schema-inject crate")?;

    let schema_lib_path = schema_crate_path
        .join("src/lib.rs");

    let mut schema_lib_contents = std::fs::read_to_string(&schema_lib_path)
        .with_context(|| "Failed reading schema-inject's lib.rs")?;

    schema_lib_contents = schema_lib_contents.replace(
        crate::INJECT_SCHEMA_MARKER,
        schema
    );

    std::fs::write(&schema_lib_path, schema_lib_contents)
        .with_context(|| "Failed writing to schema-inject's lib.rs")?;

    let schema_compilation = CompileJob::new(
        "schema-inject",
        None,
        None
    );

    let schema_compilation_results = schema_compilation.dispatch(
        "wasm32-unknown-unknown",
        None
    ).with_context(|| "Failed compiling the schema-inject crate")?;

    let schema_lib = schema_compilation_results
        .artifacts()
        .iter()
        .find(|x| x.extension().unwrap_or_default() == "a")
        .with_context(|| "Couldn't find the compiled schema-inject lib")?;

    // Build user's wasm with the schema lib statically linked
    let mut rustflags = env::var("RUSTFLAGS").unwrap_or_default();
    if !rustflags.is_empty() {
        rustflags.push(' ');
    }
    rustflags.push_str(&format!("-C link-arg={}", schema_lib.to_string_lossy()));
    
    user_lib_compilation
        .with_rustflags(rustflags)
        .dispatch("wasm32-unknown-unknown", None)
        .with_context(|| "Failed to compile user wasm")
}