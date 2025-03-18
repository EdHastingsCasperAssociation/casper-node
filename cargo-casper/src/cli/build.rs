use std::{io::Cursor, path::PathBuf, process::Command};

use anyhow::Context;

use crate::{injector, compilation::CompileJob};

/// The `build` subcommand flow.
pub fn build_impl(
    output_dir: Option<PathBuf>,
    embed_schema: bool,
) -> Result<(), anyhow::Error> {
    // Determine the output directory
    let output_dir = match output_dir {
        Some(path) => path,
        None => PathBuf::from("target/release/wasm32-unknown-unknown/")
    };

    // Build the contract package targetting wasm32-unknown-unknown without
    // extra feature flags - this is the production contract wasm file.
    //
    // Optionally (but by default) create an entrypoint in the wasm that will have
    // embedded schema JSON file for discoverability (aka internal schema).
    let compilation = CompileJob::new(
        "./Cargo.toml",
        None,
        None
    );

    // Build the contract with or without the schema.
    let production_wasm_path = if embed_schema {
        // Build the schema first
        let mut buffer = Cursor::new(Vec::new());
        super::build_schema::build_schema_impl(
            &mut buffer,
        ).context("Failed to build contract schema")?;
    
        let contract_schema = String::from_utf8(buffer.into_inner())
            .context("Failed to read contract schema")?;

        // Build the contract with above schema injected
        let production_wasm_path = injector::build_with_schema_injected(
            compilation,
            &contract_schema,
            &output_dir
        ).context("Failed compiling user wasm with schema")?;

        // Write the schema next to the wasm
        let wasm_file_name = production_wasm_path
            .with_extension("");

        let wasm_file_name = wasm_file_name
            .file_name()
            .and_then(|x| x.to_str())
            .context("Failed reading wasm file name")?;

        let schema_file_path = production_wasm_path
            .with_file_name(format!("{wasm_file_name}-schema"))
            .with_extension("json");

        std::fs::create_dir_all(&schema_file_path.parent().unwrap())
            .context("Failed creating directory for wasm schema")?;

        std::fs::write(&schema_file_path, contract_schema)
            .context("Failed writing contract schema")?;

        production_wasm_path
    } else {
        // Compile and move to specified output directory
        compilation
            .dispatch("wasm32-unknown-unknown", Option::<String>::None)
            .context("Failed to compile user wasm")?
            .flush_artifacts_to_dir(&output_dir)
            .context("Failed extracting build artifacts to directory")?
    };

    // Run wasm optimizations passes that will shrink the size of the wasm.
    Command::new("wasm-strip")
        .args(&[&production_wasm_path])
        .spawn()
        .context("Failed to execute wasm-strip command. Is wabt installed?")?;

    // Report paths
    eprintln!("Completed. Build artifacts:");
    eprintln!("{:?}", production_wasm_path.canonicalize()?);

    Ok(())
}