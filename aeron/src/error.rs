use std::ffi::NulError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("FfiError {0}")]
    FfiError(i32),
    #[error("CString NulError: {0}")]
    NulError(#[from] NulError),
}

pub(crate) fn aeron_result(code: i32) -> Result<()> {
    // TODO: aeron_errmsg
    match code {
        0 => Ok(()),
        _ => {
            println!("aeron_result: {code}");
            Err(Error::FfiError(code))
        }
    }
}
