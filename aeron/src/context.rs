use crate::{
    error::{aeron_result, Error},
    CorrelationId, SendSyncPtr, SessionId, StreamId,
};
use aeron_client_sys::{
    aeron_client_registering_resource_stct, aeron_context_close, aeron_context_get_dir,
    aeron_context_init, aeron_context_set_error_handler, aeron_context_set_on_new_publication,
    aeron_context_set_on_new_subscription, aeron_context_t,
};
use std::{
    ffi,
    ffi::{c_void, CStr, CString},
    ptr,
};

pub struct Context {
    pub(crate) inner: SendSyncPtr<aeron_context_t>,
}

impl Context {
    pub fn new() -> Result<Self, Error> {
        let mut inner = core::ptr::null_mut();
        aeron_result(unsafe { aeron_context_init(&mut inner) })?;
        debug_assert_ne!(inner, ptr::null_mut());
        Ok(Context {
            inner: inner.into(),
        })
    }

    pub fn set_error_handler<'a, F: ErrorHandler<'a>>(&self, error_handler: F) {
        let mut closure = error_handler;
        unsafe {
            aeron_context_set_error_handler(
                self.inner.as_ptr(),
                Some(error_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
        };
    }

    pub fn set_on_new_publication<F: OnNewPublication>(&self, on_new_publication: F) {
        let mut closure = on_new_publication;
        unsafe {
            aeron_context_set_on_new_publication(
                self.inner.as_ptr(),
                Some(on_new_publication_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
        };
    }

    pub fn set_on_new_subscription<F: OnNewSubscription>(&self, on_new_subscription: F) {
        let mut closure = on_new_subscription;
        unsafe {
            aeron_context_set_on_new_subscription(
                self.inner.as_ptr(),
                Some(on_new_subscription_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
        };
    }

    pub fn get_dir(&self) -> String {
        let dir = unsafe { aeron_context_get_dir(self.inner.as_ptr()) };
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

impl Drop for Context {
    fn drop(&mut self) {
        aeron_result(unsafe { aeron_context_close(self.inner.as_ptr()) }).ok(); // TODO: err
    }
}

pub trait ErrorHandler<'a>: FnMut(i32, &'a str) {}

impl<'a, F> ErrorHandler<'a> for F where F: FnMut(i32, &'a str) {}

unsafe extern "C" fn error_handler_trampoline<'a, F: ErrorHandler<'a>>(
    clientd: *mut c_void,
    code: i32,
    _message: *const i8,
) {
    // let cmsg = CStr::from_ptr(message);
    // let msg = String::from_utf8_lossy(cmsg.to_bytes()).to_string();
    let closure = &mut *(clientd as *mut F);
    closure(code, "") // TODO: err msg
}

pub trait OnNewPublication: FnMut(StreamId, SessionId, CorrelationId) {}

impl<F> OnNewPublication for F where F: FnMut(StreamId, SessionId, CorrelationId) {}

unsafe extern "C" fn on_new_publication_trampoline<F: OnNewPublication>(
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

pub trait OnNewSubscription: FnMut(StreamId, CorrelationId) {}

impl<F> OnNewSubscription for F where F: FnMut(StreamId, CorrelationId) {}

unsafe extern "C" fn on_new_subscription_trampoline<F: OnNewSubscription>(
    clientd: *mut c_void,
    _handle: *mut aeron_client_registering_resource_stct,
    _channel: *const i8,
    stream_id: i32,
    correlation_id: i64,
) {
    let closure = &mut *(clientd as *mut F);
    closure(StreamId(stream_id), CorrelationId(correlation_id));
}
