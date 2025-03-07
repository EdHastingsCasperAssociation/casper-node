use anyhow::Context;
use casper_sdk::abi::Definitions;
use compilation::CompileJob;

use std::{ffi::c_void, fs, path::PathBuf, ptr::NonNull, str::FromStr};

use clap::{Parser, Subcommand};

pub(crate) mod compilation;
pub(crate) mod injector;

const INJECT_SCHEMA_MARKER: &str = "{{__CARGO_CASPER_INJECT_SCHEMA_MARKER}}";

#[derive(Debug, Subcommand)]
pub enum Command {
    GetSchema {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[command(flatten)]
        manifest: clap_cargo::Manifest,
        #[command(flatten)]
        workspace: clap_cargo::Workspace,
        #[command(flatten)]
        features: clap_cargo::Features,
    },
}

#[derive(Debug, clap::Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

type CasperLoadEntrypoints = unsafe extern "C" fn(
    unsafe extern "C" fn(*const casper_sdk::schema::SchemaEntryPoint, usize, *mut c_void),
    *mut c_void,
);
type CollectABI = unsafe extern "C" fn(*mut casper_sdk::abi::Definitions);

unsafe extern "C" fn load_entrypoints_cb(
    entrypoint: *const casper_sdk::schema::SchemaEntryPoint,
    count: usize,
    ctx: *mut c_void,
) {
    let slice = unsafe { std::slice::from_raw_parts(entrypoint, count) };
    // pass it to ctx
    let ctx = unsafe { &mut *(ctx as *mut Vec<casper_sdk::schema::SchemaEntryPoint>) };
    ctx.extend_from_slice(slice);
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        // TODO: This is called *Get*Schema, but on top of that, it also
        // produces a production-ready contract with said schema embedded...
        //
        // I'd consider either changing the name of this to something more fitting
        // or extracting the creation of schema-embedded prod contracts someplace
        // else entirely (eg. BuildSchema with an optional --embedded flag sounds more appropriate).
        Command::GetSchema {
            output: output_path,
            features,
            ..
        } => {
            // Stage 1: Compile contract package to a native library with extra code that will
            // produce ABI information including entrypoints, types, etc.
            let compilation = CompileJob::new(
                "./Cargo.toml",
                Some(features.features.clone()),
                Some("-C link-dead-code".into())
            );
            let build_result = compilation.dispatch(
                env!("TARGET"), [
                    "casper-sdk/__abi_generator".to_string(),
                    "casper-macros/__abi_generator".to_string(),
                ]
            ).with_context(|| "ABI-rich wasm compilation failure")?;

            // Stage 2: Extract ABI information from the built contract
            let artifact_path = build_result
                .artifacts()
                .into_iter()
                .find(|x| x.extension().unwrap_or_default() == "so")
                .with_context(|| "Failed loading the built contract")?;
            eprintln!("Loading: {artifact_path:?}");

            let lib = unsafe { libloading::Library::new(&artifact_path).unwrap() };

            let load_entrypoints: libloading::Symbol<CasperLoadEntrypoints> =
                unsafe { lib.get(b"__cargo_casper_load_entrypoints").unwrap() };

            let collect_abi: libloading::Symbol<CollectABI> =
                unsafe { lib.get(b"__cargo_casper_collect_abi").unwrap() };

            let entry_points = {
                let mut entrypoints: Vec<casper_sdk::schema::SchemaEntryPoint> = Vec::new();
                let ctx = NonNull::from(&mut entrypoints);
                unsafe { load_entrypoints(load_entrypoints_cb, ctx.as_ptr() as _) };
                entrypoints
            };

            let defs = {
                // TODO: segfaults

                /*let mut defs = casper_sdk::abi::Definitions::default();
                let ptr = NonNull::from(&mut defs);
                unsafe {
                    collect_abi(ptr.as_ptr());
                }
                defs*/

                Definitions::default()
            };

            // Stage 3: Construct a schema object from the extracted information
            // TODO: Move schema outside sdk to avoid importing unnecessary deps into wasm build
            let schema = casper_sdk::schema::Schema {
                name: "contract".to_string(),
                version: None,
                type_: casper_sdk::schema::SchemaType::Contract {
                    state: "Contract".to_string(), /* TODO: This is placeholder, do we need to
                                                    * extract this? */
                },
                definitions: defs,
                entry_points,
            };

            if let Some(output) = &output_path {
                let mut file = fs::File::create(output)?;
                serde_json::to_writer_pretty(&mut file, &schema)?;
            } else {
                serde_json::to_writer_pretty(std::io::stdout(), &schema)?;
            }

            // Stage 4: Build the contract package again, but now using wasm32-unknown-unknown
            // target without extra feature flags - this is the production contract wasm file.
            //
            // Optionally (but by default) create an entrypoint in the wasm that will have
            // embedded schema JSON file for discoverability (aka internal schema).
            let wasm_output = match output_path {
                Some(path) => path,
                None => PathBuf::from_str("./").unwrap(),
            };

            let production_wasm_path = injector::build_with_schema_injected(
                compilation,
                &serde_json::to_string(&schema)?,
                &wasm_output
            ).with_context(|| "Failed compiling user wasm with schema")?;

            // Stage 5: Run wasm optimizations passes that will shrink the size of the wasm.
            std::process::Command::new("wasm-strip")
                .args(&[&production_wasm_path])
                .spawn()
                .with_context(|| "Failed to execute wasm-strip command. Is wabt installed?")?;

            // Stage 6: Update external schema file by adding wasm hash from Stage 4.
            // TODO: The above

            // Stage 7: Report all paths
            eprintln!("Production wasm at: {wasm_output:?}");
        }
    }
    Ok(())
}
