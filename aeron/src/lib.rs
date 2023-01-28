pub mod client;
pub mod context;
pub mod error;
pub mod publication;
pub mod subscription;

use aeron_client_sys::aeron_header_values_t;
use std::ptr::NonNull;

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

#[derive(Copy, Clone, Debug)]
#[repr(i64)]
pub enum ChannelStatus {
    Active = 1,
    Errored = -1,
    Other(i64),
}

#[derive(Clone, Debug)]
pub struct Header(aeron_header_values_t);

impl Header {
    pub fn version(&self) -> i8 {
        self.0.frame.version
    }

    pub fn flags(&self) -> u8 {
        self.0.frame.flags
    }

    pub fn r#type(&self) -> HeaderType {
        HeaderType(self.0.frame.type_)
    }

    pub fn session_id(&self) -> SessionId {
        SessionId(self.0.frame.session_id)
    }

    pub fn stream_id(&self) -> StreamId {
        StreamId(self.0.frame.stream_id)
    }

    pub fn term_id(&self) -> TermId {
        TermId(self.0.frame.term_id)
    }

    pub fn term_offset(&self) -> i32 {
        self.0.frame.term_offset
    }

    pub fn reserved_value(&self) -> i64 {
        self.0.frame.reserved_value
    }
}

#[derive(Copy, Clone, Debug)]
pub struct HeaderType(i16);

#[allow(non_upper_case_globals)]
impl HeaderType {
    pub const PaddingFrame: Self = Self(0x0000);
    pub const DataFragment: Self = Self(0x0001);
    pub const NAK: Self = Self(0x0002);
    pub const SM: Self = Self(0x0003);
    pub const Error: Self = Self(0x0004);
    pub const Setup: Self = Self(0x0005);
    pub const RTTM: Self = Self(0x0006);
    pub const Resolution: Self = Self(0x0007);
    pub const Extension: Self = Self(-1);
}

#[repr(transparent)]
pub(crate) struct SendSyncPtr<T>(NonNull<T>);

unsafe impl<T> Send for SendSyncPtr<T> {
    // TODO: verify that the C client doesn't use TLS.
}

unsafe impl<T> Sync for SendSyncPtr<T> {
    // TODO: confirm that it's okay to make a read-only ptr Sync.
}

impl<T> SendSyncPtr<T> {
    #[inline(always)]
    pub const fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T> From<*mut T> for SendSyncPtr<T> {
    #[inline(always)]
    fn from(inner: *mut T) -> Self {
        debug_assert!(!inner.is_null());
        SendSyncPtr(unsafe { NonNull::new_unchecked(inner) })
    }
}
