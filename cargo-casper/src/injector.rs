use anyhow::Context;
use include_dir::{include_dir, Dir};
use std::path::PathBuf;

use crate::{cli::extract_embedded_dir, compilation::CompileJob};

// Embed the entire "injected" directory into the binary.
static INJECTED_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/schema-inject");

const INJECT_SCHEMA_MARKER: &str = "{{__CARGO_CASPER_INJECT_SCHEMA_MARKER}}";

/// Builds the specified WASM, while also injecting the specified schema
/// into it.
pub fn build_with_schema_injected(
    user_lib_compilation: CompileJob,
    schema: &str,
) -> Result<PathBuf, anyhow::Error> {
    // Compile the schema-inject crate, with the appropriate schema string
    // injected into it.
    let schema_crate_path = extract_embedded_dir(&PathBuf::from("./target/.schema"), &INJECTED_DIR)
        .with_context(|| "Failed extracting the schema-inject crate")?;

    let schema_lib_path = schema_crate_path.join("src/lib.rs");

    let mut schema_lib_contents = std::fs::read_to_string(&schema_lib_path)
        .with_context(|| "Failed reading schema-inject's lib.rs")?;

    schema_lib_contents = schema_lib_contents.replace(INJECT_SCHEMA_MARKER, schema);

    std::fs::write(&schema_lib_path, schema_lib_contents)
        .with_context(|| "Failed writing to schema-inject's lib.rs")?;

    let schema_compilation = CompileJob::new(None, None, None).in_directory(schema_crate_path);

    let schema_compilation_results = schema_compilation
        .dispatch("wasm32-unknown-unknown", Option::<String>::None)
        .with_context(|| "Failed compiling the schema-inject crate")?;

    let schema_lib = schema_compilation_results
        .artifacts()
        .iter()
        .find(|x| x.extension().unwrap_or_default() == "a")
        .context("Couldn't find the compiled schema-inject lib")?
        .canonicalize()
        .context("Couldn't resolve compiled schema's path")?;

    // Build user's wasm with the schema lib statically linked and exported
    let rustflags = format!(
        "-C link-arg={} -C link-arg=--export=__casper_schema",
        schema_lib.to_string_lossy()
    );

    let compilation_results = user_lib_compilation
        .with_rustflags(rustflags)
        .dispatch("wasm32-unknown-unknown", Option::<String>::None)
        .context("Failed to compile user wasm")?;

    let production_wasm_path = compilation_results
        .get_artifact_by_extension("wasm")
        .context("Build artifacts for contract wasm didn't include a wasm file")?;

    Ok(production_wasm_path.into())
}
