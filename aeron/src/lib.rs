pub mod client;
pub mod context;
pub mod error;
pub mod publication;
pub mod subscription;

use std::ptr;

#[derive(Copy, Clone, Debug)]
pub struct StreamId(pub i32);

#[derive(Copy, Clone, Debug)]
pub struct SessionId(pub i32);

#[derive(Copy, Clone, Debug)]
pub struct CorrelationId(pub i64);

#[derive(Copy, Clone, Debug)]
pub struct TermId(pub i32);

#[derive(Copy, Clone, Debug)]
pub struct Position(pub i64);

#[repr(i64)]
pub enum ChannelStatus {
    Active = 1,
    Errored = -1,
    Other(i64),
}

#[repr(transparent)]
pub(crate) struct SendSyncPtr<T>(*mut T);

unsafe impl<T> Send for SendSyncPtr<T> {
    // TODO: verify that the C client doesn't use TLS.
}

unsafe impl<T> Sync for SendSyncPtr<T> {
    // TODO: confirm that it's okay to make a read-only ptr Sync.
}

impl<T> SendSyncPtr<T> {
    pub const fn as_ptr(&self) -> *mut T {
        self.0 as *mut T
    }
}

impl<T> From<*mut T> for SendSyncPtr<T> {
    fn from(inner: *mut T) -> Self {
        debug_assert_ne!(inner, ptr::null_mut());
        SendSyncPtr(inner)
    }
}
