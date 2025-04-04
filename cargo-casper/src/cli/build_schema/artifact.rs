use core::slice;
use std::{ffi::c_void, path::Path, ptr::NonNull};

use libloading::{Library, Symbol};

const COLLECT_SCHEMA_FUNC: &str = "__cargo_casper_collect_schema";

type CollectSchema = unsafe extern "C" fn(
    callback: unsafe extern "C" fn(*const u8, usize, *mut c_void),
    ctx: *mut c_void,
);

pub(crate) struct Artifact {
    library: Library,
}

unsafe extern "C" fn collect_schema_cb<T: FnOnce(&[u8])>(
    data_ptr: *const u8,
    data_len: usize,
    ctx: *mut c_void,
) {
    let ptr = ctx.cast::<Option<T>>();
    let ptr = (*ptr).take().unwrap();
    let data = slice::from_raw_parts(data_ptr, data_len);
    ptr(data);
}

fn collect_schema_helper<T>(library: &Library, cb: T)
where
    T: for<'a> FnOnce(&'a [u8]),
{
    let collect_schema: Symbol<CollectSchema> =
        unsafe { library.get(COLLECT_SCHEMA_FUNC.as_bytes()).unwrap() };

    // We need to wrap the callback in an `Option` to make sure it is not dropped
    // before the callback is called.
    let wrapped_cb = Some(cb);
    let ptr = NonNull::from(&wrapped_cb);

    unsafe {
        collect_schema(
            collect_schema_cb::<T>,
            ptr.cast::<c_void>().as_ptr(),
        )
    };
}

impl Artifact {
    pub(crate) fn from_path<P: AsRef<Path>>(
        artifact_path: P,
    ) -> Result<Artifact, libloading::Error> {
        let library = unsafe { libloading::Library::new(artifact_path.as_ref()) }?;

        Ok(Self { library })
    }

    /// Collects schema from the built artifact.
    ///
    /// This returns a [`serde_json::Value`] to skip validation of a `Schema` object structure which
    /// (in theory) can differ.
    pub(crate) fn collect_schema(&self) -> serde_json::Result<serde_json::Value> {
        let mut value = None;

        collect_schema_helper(&self.library, |data| {
            println!("Inside callback {:?}", std::str::from_utf8(data));
            value = Some(serde_json::from_slice(data));
        });

        let result = value.expect("Callback called");

        eprintln!("{result:?}");

        result
    }
}
