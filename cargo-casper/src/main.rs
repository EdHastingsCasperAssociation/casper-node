use anyhow::{bail, Context};
use tempfile::TempDir;

use std::{ffi::c_void, fs, path::PathBuf, ptr::NonNull};

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

struct Compilation<'a> {
    workspace: &'a clap_cargo::Workspace,
    manifest: &'a clap_cargo::Manifest,
    features: &'a clap_cargo::Features
}

struct CompilationResults {
    artifacts: Vec<PathBuf>,
}

impl<'a> Compilation<'a> {
    pub fn new(
        workspace: &'a clap_cargo::Workspace,
        manifest: &'a clap_cargo::Manifest,
        features: &'a clap_cargo::Features
    ) -> Self {
        Self {
            workspace,
            manifest,
            features
        }
    }

    pub fn build<T: IntoIterator<Item = String>>(
        &self,
        target: &'static str,
        extra_features: T
    ) -> Result<CompilationResults, anyhow::Error> {
        let tempdir = TempDir::new()
            .with_context(|| "Failed to create temporary directory")?;

        let package_name = self.workspace.package.first()
            .with_context(|| "The workspace doesn't contain a package definition")?;

        let mut features = self.features.clone();
        features.features.extend(extra_features);

        let features_str = features.features.join(",");

        let mut args = vec!["build", "-p", package_name.as_str()];
        args.extend(["--target", target]);
        args.extend(["--features", &features_str, "--lib", "--release"]);
        args.extend([
            "--target-dir",
            &tempdir.path().as_os_str().to_str().expect("invalid path"),
        ]);

        eprintln!("Running command {:?}", args);
        let mut output = std::process::Command::new("cargo")
            .args(&args)
            .spawn()
            .with_context(|| "Failed to execute command")?;

        let exit_status = output
            .wait()
            .with_context(|| "Failed to wait on child")?;

        if !exit_status.success() {
            eprintln!("Command executed with failing error code");
            std::process::exit(exit_status.code().unwrap_or(1));
        }

        let artifact_dir = tempdir.path().join(target).join("release");

        let artifacts: Vec<_> = fs::read_dir(&artifact_dir)
            .with_context(|| "Artifact read directory failure")?
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

        Ok(CompilationResults {
            artifacts
        })
    }
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
            manifest,
            workspace,
            features,
        } => {
            // Stage 1: Compile contract package to a native library with extra code that will
            // produce ABI information including entrypoints, types, etc.
            let compilation = Compilation::new(&workspace, &manifest, &features);
            let build_result = compilation.build(
                env!("TARGET"), [
                    "casper-sdk/__abi_generator".to_string(),
                    "casper-macros/__abi_generator".to_string(),
                ]
            ).with_context(|| "ABI-rich wasm compilation failure")?;

            // Stage 2: Extract ABI information from the built contract
            let artifact_path = build_result.artifacts.into_iter().next().expect("artifact");

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
                let mut defs = casper_sdk::abi::Definitions::default();
                let ptr = NonNull::from(&mut defs);
                unsafe {
                    collect_abi(ptr.as_ptr());
                }
                defs
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

            if let Some(output) = output_path {
                let mut file = fs::File::create(&output)?;
                serde_json::to_writer_pretty(&mut file, &schema)?;
            } else {
                serde_json::to_writer_pretty(std::io::stdout(), &schema)?;
            }

            // Stage 4: Build the contract package again, but now using wasm32-unknown-unknown
            // target without extra feature flags - this is the production contract wasm file.
            let build_result = compilation.build("wasm32-unknown-unknown", [])
                .with_context(|| "ABI-rich wasm compilation failure")?;
            
            // Stage 4a: Optionally (but by default) create an entrypoint in the wasm that will have
            // embedded schema JSON file for discoverability (aka internal schema).

            // Stage 5: Run wasm optimizations passes that will shrink the size of the wasm.
            

            // Stage 6: Update external schema file by adding wasm hash from Stage 4.
            let artifact: &PathBuf = build_result.artifacts.get(0).unwrap();
            

            // Stage 7: Report all paths

        }
    }
    Ok(())
}
