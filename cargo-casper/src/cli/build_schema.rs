use std::{ffi::c_void, io::Write, ptr::NonNull};

use anyhow::Context;
use casper_sdk::{abi::Definitions, schema::{Schema, SchemaEntryPoint, SchemaType}};
use libloading::{Library, Symbol};

use crate::CompileJob;

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

/// The `build-schema` subcommand flow. The schema is written to the specified
/// [`Write`] implementer.
pub fn build_schema_impl<W: Write>(
    output_writer: &mut W,
    features: clap_cargo::Features
) -> Result<(), anyhow::Error> {
    // Compile contract package to a native library with extra code that will
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
    ).context("ABI-rich wasm compilation failure")?;

    // Extract ABI information from the built contract
    let artifact_path = build_result
        .artifacts()
        .into_iter()
        .find(|x| x.extension().unwrap_or_default() == "so")
        .context("Failed loading the built contract")?;
    eprintln!("Loading: {artifact_path:?}");

    let lib = unsafe { Library::new(&artifact_path).unwrap() };

    let load_entrypoints: Symbol<CasperLoadEntrypoints> =
        unsafe { lib.get(b"__cargo_casper_load_entrypoints").unwrap() };

    #[allow(unused_variables)]
    let collect_abi: Symbol<CollectABI> =
        unsafe { lib.get(b"__cargo_casper_collect_abi").unwrap() };

    let entry_points = {
        let mut entrypoints: Vec<SchemaEntryPoint> = Vec::new();
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

    // Construct a schema object from the extracted information
    // TODO: Move schema outside sdk to avoid importing unnecessary deps into wasm build
    let schema = Schema {
        name: "contract".to_string(),
        version: None,
        type_: SchemaType::Contract {
            state: "Contract".to_string(), /* TODO: This is placeholder, do we need to
                                        * extract this? */
        },
        definitions: defs,
        entry_points,
    };

    serde_json::to_writer_pretty(output_writer, &schema)
        .context("Failed writing schema")?;

    Ok(())
}