use std::{ffi::c_void, io::Write, ptr::NonNull};

use anyhow::Context;
use casper_sdk::{
    abi_generator::Message,
    schema::{Schema, SchemaMessage, SchemaType},
};

use crate::compilation::CompileJob;

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

/// The `build-schema` subcommand flow. The schema is written to the specified
/// [`Write`] implementer.
pub fn build_schema_impl<W: Write>(output_writer: &mut W) -> Result<(), anyhow::Error> {
    // Compile contract package to a native library with extra code that will
    // produce ABI information including entrypoints, types, etc.
    eprintln!("Building contract schema...");
    let compilation = CompileJob::new("./Cargo.toml", None, Some("-C link-dead-code".into()));

    let build_result = compilation
        .dispatch(
            env!("TARGET"),
            [
                "casper-sdk/__abi_generator".to_string(),
                "casper-macros/__abi_generator".to_string(),
            ],
        )
        .context("ABI-rich wasm compilation failure")?;

    // Extract ABI information from the built contract
    let artifact_path = build_result
        .artifacts()
        .into_iter()
        .find(|x| x.extension().unwrap_or_default() == "so")
        .context("Failed loading the built contract")?;

    let lib = unsafe { libloading::Library::new(&artifact_path).unwrap() };

    let load_entrypoints: libloading::Symbol<CasperLoadEntrypoints> =
        unsafe { lib.get(b"__cargo_casper_load_entrypoints").unwrap() };

    #[allow(unused_variables)]
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
        // TODO: This segfaults

        /*let mut defs = casper_sdk::abi::Definitions::default();
        let ptr = NonNull::from(&mut defs);
        unsafe {
            collect_abi(ptr.as_ptr());
        }
        defs*/

        Default::default()
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

    // Construct a schema object from the extracted information
    let schema = Schema {
        name: "contract".to_string(),
        version: None,
        type_: SchemaType::Contract {
            state: "Contract".to_string(),
        },
        definitions: defs,
        entry_points,
        messages,
    };

    // Write the schema using the provided writer
    serde_json::to_writer_pretty(output_writer, &schema).context("Failed writing schema")?;

    Ok(())
}
