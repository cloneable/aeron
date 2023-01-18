use crate::{
    error::{aeron_result, Error},
    CorrelationId, SessionId, StreamId,
};
use aeron_client_sys::{
    aeron_client_registering_resource_stct, aeron_context_close, aeron_context_get_dir,
    aeron_context_init, aeron_context_set_error_handler, aeron_context_set_on_new_publication,
    aeron_context_t,
};
use std::{
    ffi,
    ffi::{c_void, CStr, CString},
    ptr,
};

pub struct Context {
    pub(crate) inner: *mut aeron_context_t,
}

unsafe impl Send for Context {
    // TODO: verify that the C client doesn't use TLS
}

impl Context {
    pub fn new() -> Result<Self, Error> {
        let mut inner = core::ptr::null_mut();
        aeron_result(unsafe { aeron_context_init(&mut inner) })?;
        debug_assert_ne!(inner, ptr::null_mut());

        let ctx = Context { inner };

        aeron_result(unsafe {
            aeron_context_set_error_handler(ctx.inner, Some(error_handler), ptr::null_mut())
        })?;

        Ok(ctx)
    }

    pub fn set_on_new_publication<F: OnNewPublication>(&self, on_new_publication: F) {
        let mut closure = on_new_publication;
        unsafe {
            aeron_context_set_on_new_publication(
                self.inner,
                Some(on_new_publication_handler::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
        };
    }

    pub fn get_dir(&self) -> String {
        let dir = unsafe { aeron_context_get_dir(self.inner) };
        if dir != ptr::null() {
            unsafe {
                let cs = CStr::from_ptr(dir as *mut i8);
                CString::from(cs).into_string().expect("string")
            }
        } else {
            "".to_owned()
        }
    }
}

unsafe extern "C" fn error_handler(_clientd: *mut c_void, code: i32, message: *const i8) {
    let msg = CStr::from_ptr(message).to_string_lossy();
    println!("ERR{code}: {msg}");
}

impl Drop for Context {
    fn drop(&mut self) {
        aeron_result(unsafe { aeron_context_close(self.inner) }).ok(); // TODO: err
    }
}

pub trait OnNewPublication: FnMut(StreamId, SessionId, CorrelationId) {}

impl<F> OnNewPublication for F where F: FnMut(StreamId, SessionId, CorrelationId) {}

unsafe extern "C" fn on_new_publication_handler<F: OnNewPublication>(
    clientd: *mut c_void,
    _handle: *mut aeron_client_registering_resource_stct,
    _channel: *const i8,
    stream_id: i32,
    session_id: i32,
    correlation_id: i64,
) {
    let closure = &mut *(clientd as *mut F);
    closure(
        StreamId(stream_id),
        SessionId(session_id),
        CorrelationId(correlation_id),
    );
}
