use std::{io::Cursor, path::PathBuf, process::Command};

use anyhow::Context;

use crate::{injector, compilation::CompileJob};

/// The `build` subcommand flow.
pub fn build_impl(
    output_dir: Option<PathBuf>,
    embed_schema: bool,
) -> Result<(), anyhow::Error> {
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
        eprintln!("Building contract with schema injected...");
        let production_wasm_path = injector::build_with_schema_injected(
            compilation,
            &contract_schema,
        ).context("Failed compiling user wasm with schema")?;

        // Write the schema next to the wasm
        let schema_file_path = production_wasm_path
            .with_extension("json");

        std::fs::create_dir_all(&schema_file_path.parent().unwrap())
            .context("Failed creating directory for wasm schema")?;

        std::fs::write(&schema_file_path, contract_schema)
            .context("Failed writing contract schema")?;

        production_wasm_path
    } else {
        // Compile and move to specified output directory
        eprintln!("Building contract...");
        compilation
            .dispatch("wasm32-unknown-unknown", Option::<String>::None)
            .context("Failed to compile user wasm")?
            .get_artifact_by_extension("wasm")
            .context("Failed extracting build artifacts to directory")?
    };

    // Run wasm optimizations passes that will shrink the size of the wasm.
    eprintln!("Applying optimizations...");
    Command::new("wasm-strip")
        .args(&[&production_wasm_path])
        .spawn()
        .context("Failed to execute wasm-strip command. Is wabt installed?")?;

    // Copy to output_dir if specified
    let mut out_wasm_path = production_wasm_path.clone();
    let mut out_schema_path = None;

    if let Some(output_dir) = output_dir {
        out_wasm_path = output_dir.join(out_wasm_path.file_stem().unwrap()).join("wasm");
        std::fs::copy(&production_wasm_path, &out_wasm_path)
            .context("Couldn't write to the specified output directory.")?;
    }

    if embed_schema {
        out_schema_path = Some(out_wasm_path.with_extension("json"));
        let production_schema_path = production_wasm_path.with_extension("json");
        std::fs::copy(&production_schema_path, out_schema_path.as_ref().unwrap())
            .context("Couldn't write to the specified output directory.")?;
    }

    // Report paths
    eprintln!("Completed. Build artifacts:");
    eprintln!("{:?}", out_wasm_path.canonicalize()?);
    if let Some(schema_path) = out_schema_path {
        eprintln!("{:?}", schema_path.canonicalize()?);
    }

    Ok(())
}