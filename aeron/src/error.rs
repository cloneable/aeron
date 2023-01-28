use aeron_client_sys as sys;
use std::ffi::{CStr, NulError};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("FfiError {0}: {1}")]
    FfiError(i32, String),
    #[error("CString NulError: {0}")]
    NulError(#[from] NulError),
}

pub(crate) fn aeron_result(code: i32) -> Result<()> {
    // TODO: aeron_errmsg
    match code {
        0 => Ok(()),
        _ => Err(aeron_error(code)),
    }
}

pub(crate) fn aeron_error(code: i32) -> Error {
    let msg = unsafe { CStr::from_ptr(sys::aeron_errmsg()) }.to_string_lossy();
    println!("aeron_result: {code}");
    Error::FfiError(code, msg.to_string())
}
