use anyhow::{bail, Context};
use casper_sdk::{abi_generator::Message, schema::SchemaMessage};

use clap::Parser;
use cli::{Cli, Command};
use compilation::CompileJob;


use std::{fs::File, io::Write};


use clap::{Parser, Subcommand};

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
// ...
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
type CollectMessages = unsafe extern "C" fn(
    callback: unsafe extern "C" fn(*const Message, usize, *mut c_void),
    ctx: *mut c_void,
);

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

pub(crate) mod compilation;
pub(crate) mod injector;
pub(crate) mod cli;


unsafe extern "C" fn collect_messages_cb(messages: *const Message, count: usize, ctx: *mut c_void) {
    let slice = unsafe { std::slice::from_raw_parts(messages, count) };
    // pass it to ctx
    let ctx = unsafe { &mut *(ctx as *mut Vec<SchemaMessage>) };

    for message in slice {
        let schema_message = SchemaMessage {
            name: message.name.to_string(),
            decl: message.decl.to_string(),
        };
        ctx.push(schema_message);
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::BuildSchema { 
            output,
            features,
            ..
        } => {

            //
            // Stage 1: compile contract package to a native library with extra code that will
            // produce ABI information including entrypoints, types, etc.
            //
            let tempdir = tempfile::TempDir::new().expect("Failed to create tempdir");

            let target_platform = env!("TARGET");

            let package_name = workspace.package.first().expect("no package");

            let extra_features = ["casper-sdk/__abi_generator".to_string()];
            features.features.extend(extra_features);

            let features_str = features.features.join(",");

            let mut args = vec!["build", "-p", package_name.as_str()];

            args.extend(["--target", target_platform]);
            args.extend(["--features", &features_str, "--lib", "--release"]);
            args.extend([
                "--target-dir",
                &tempdir.path().as_os_str().to_str().expect("invalid path"),
            ]);
            eprintln!("Running command {:?}", args);

            let mut output = std::process::Command::new("cargo")
                .args(&args)
                .spawn()
                .expect("Failed to execute command");
            let exit_status = output.wait().expect("Failed to wait on child");
            if !exit_status.success() {
                eprintln!("Command executed with failing error code");
                std::process::exit(exit_status.code().unwrap_or(1));
            }

            let artifact_dir = tempdir.path().join(target_platform).join("release");

            let artifacts: Vec<_> = fs::read_dir(&artifact_dir)
                .with_context(|| "Read directory")?
                .into_iter()
                .filter_map(|dir_entry| {
                    let dir_entry = dir_entry.unwrap();
                    let path = dir_entry.path();
                    if path.is_file()
                        && dbg!(&path)
                            .extension()?
                            .to_str()
                            .expect("valid string")
                            .ends_with(&std::env::consts::DLL_SUFFIX[1..])
                    {
                        Some(path)
                    } else {
                        None
                    }
                })
                .collect();

            if artifacts.len() != 1 {
                bail!("Expected exactly one build artifact: {:?}", artifacts);
            }

            let artifact_path = artifacts.into_iter().next().expect("artifact");

            let lib = unsafe { libloading::Library::new(&artifact_path).unwrap() };

            let load_entrypoints: libloading::Symbol<CasperLoadEntrypoints> =
                unsafe { lib.get(b"__cargo_casper_load_entrypoints").unwrap() };
            let collect_abi: libloading::Symbol<CollectABI> =
                unsafe { lib.get(b"__cargo_casper_collect_abi").unwrap() };
            let collect_messages: libloading::Symbol<CollectMessages> =
                unsafe { lib.get(b"__cargo_casper_collect_messages").unwrap() };

            let entry_points = {
                let mut entrypoints: Vec<casper_sdk::schema::SchemaEntryPoint> = Vec::new();
                let ctx: NonNull<Vec<casper_sdk::schema::SchemaEntryPoint>> =
                    NonNull::from(&mut entrypoints);
                unsafe { load_entrypoints(load_entrypoints_cb, ctx.as_ptr() as _) };
                entrypoints
            };

            let defs = {
                let mut defs = casper_sdk::abi::Definitions::default();
                let ptr = NonNull::from(&mut defs);
                unsafe {
                    collect_abi(ptr.as_ptr());
                }
                defs
            };

            let messages = {
                let mut messages: Vec<SchemaMessage> = Vec::new();
                unsafe {
                    collect_messages(
                        collect_messages_cb,
                        NonNull::from(&mut messages).as_ptr() as _,
                    );
                }
                messages
            };

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
                messages,
            };

            if let Some(output) = output_path {
                let mut file = fs::File::create(&output)?;
                serde_json::to_writer_pretty(&mut file, &schema)?;
            } else {
                serde_json::to_writer_pretty(std::io::stdout(), &schema)?;
            }

            //
            // Stage 2: Construct a schema object from the extracted information
            //

            // Stage 3: Build the contract package again, but now using wasm32-unknown-unknown
            // target without extra feature flags - this is the production contract wasm file.
            // Stage 3a: Optionally (but by default) create an entrypoint in the wasm that will have
            // embedded schema JSON file for discoverability (aka internal schema).
            // Stage 3b: Run wasm optimizations passes that will shrink the size of the wasm.

            //
            // Stage 4: Update external schema file by adding wasm hash from Stage 3.
            //

            // Stage 5: Report all paths
        }

            // If user specified an output path, write there.
            // Otherwise print to standard output.
            let mut schema_writer: Box<dyn Write> = match output {
                Some(path) => Box::new(File::create(path)?),
                None => Box::new(std::io::stdout()),
            };

            cli::build_schema::build_schema_impl(
                &mut schema_writer,
                features
            )?
        },
        Command::Build { 
            output,
            features,
            ..
        } => {
            cli::build::build_impl(
                output,
                features
            )?
        },

    }
    Ok(())
}
