use crate::{
    error::{aeron_result, Error},
    CorrelationId, SendSyncPtr, SessionId, StreamId,
};
use aeron_client_sys as sys;
use std::{
    ffi,
    ffi::{c_void, CStr, CString},
    ptr,
};

pub struct Context {
    pub(crate) inner: SendSyncPtr<sys::aeron_context_t>,
}

impl Context {
    pub fn new() -> Result<Self, Error> {
        let mut inner = ptr::null_mut();
        aeron_result(unsafe { sys::aeron_context_init(&mut inner) })?;
        Ok(Context { inner: inner.into() })
    }

    pub fn set_error_handler<F>(&self, error_handler: F)
    where
        F: for<'a> FnMut(i32, &'a str),
    {
        let mut closure = error_handler;
        unsafe {
            sys::aeron_context_set_error_handler(
                self.inner.as_ptr(),
                Some(error_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
        };
    }

    pub fn set_on_new_publication<F>(&mut self, on_new_publication: F)
    where
        F: for<'a> FnMut(&'a str, StreamId, SessionId, CorrelationId),
    {
        let mut closure = on_new_publication;
        unsafe {
            sys::aeron_context_set_on_new_publication(
                self.inner.as_ptr(),
                Some(on_new_publication_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
        };
    }

    pub fn set_on_new_subscription<F>(&mut self, on_new_subscription: F)
    where
        F: for<'a> FnMut(&'a str, StreamId, CorrelationId),
    {
        let mut closure = on_new_subscription;
        unsafe {
            sys::aeron_context_set_on_new_subscription(
                self.inner.as_ptr(),
                Some(on_new_subscription_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
        };
    }

    pub fn get_dir(&self) -> String {
        let dir = unsafe { sys::aeron_context_get_dir(self.inner.as_ptr()) };
        if !dir.is_null() {
            unsafe {
                let cs = CStr::from_ptr(dir as *mut i8);
                CString::from(cs).into_string().expect("string")
            }
        } else {
            "".to_owned()
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // TODO: err
        aeron_result(unsafe { sys::aeron_context_close(self.inner.as_ptr()) }).ok();
    }
}

unsafe extern "C" fn error_handler_trampoline<F>(
    clientd: *mut c_void,
    code: i32,
    message: *const i8,
) where
    F: for<'a> FnMut(i32, &'a str),
{
    let message = &*CStr::from_ptr(message).to_string_lossy();
    let closure = &mut *(clientd as *mut F);
    closure(code, message)
}

unsafe extern "C" fn on_new_publication_trampoline<F>(
    clientd: *mut c_void,
    _handle: *mut sys::aeron_client_registering_resource_stct,
    channel: *const i8,
    stream_id: i32,
    session_id: i32,
    correlation_id: i64,
) where
    F: for<'a> FnMut(&'a str, StreamId, SessionId, CorrelationId),
{
    let channel = &*CStr::from_ptr(channel).to_string_lossy();
    let closure = &mut *(clientd as *mut F);
    closure(channel, StreamId(stream_id), SessionId(session_id), CorrelationId(correlation_id));
}

unsafe extern "C" fn on_new_subscription_trampoline<F>(
    clientd: *mut c_void,
    _handle: *mut sys::aeron_client_registering_resource_stct,
    channel: *const i8,
    stream_id: i32,
    correlation_id: i64,
) where
    F: for<'a> FnMut(&'a str, StreamId, CorrelationId),
{
    let channel = &*CStr::from_ptr(channel).to_string_lossy();
    let closure = &mut *(clientd as *mut F);
    closure(channel, StreamId(stream_id), CorrelationId(correlation_id));
}
