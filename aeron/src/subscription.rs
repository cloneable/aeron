use crate::{
    client::Aeron,
    error::{aeron_error, aeron_result, Result},
    ChannelStatus, Header, SendSyncPtr, SessionId, StreamId, TermId,
};
use aeron_client_sys as sys;
use std::{
    ffi,
    ffi::CString,
    mem::MaybeUninit,
    slice,
    {future::Future, pin::Pin, sync::Arc, task::Poll},
    {ptr, task},
};

pub struct Subscription {
    client: Arc<Aeron>,
    inner: SendSyncPtr<sys::aeron_subscription_t>,
}

impl Subscription {
    fn new(client: &Arc<Aeron>, inner: *mut sys::aeron_subscription_t) -> Self {
        Subscription { client: client.clone(), inner: inner.into() }
    }

    pub fn add_destination(self: &Arc<Self>, uri: &str) -> Result<AsyncDestination> {
        let uri = CString::new(uri.as_bytes())?;
        let mut inner = ptr::null_mut();
        aeron_result(unsafe {
            sys::aeron_subscription_async_add_destination(
                &mut inner,
                self.client.inner.as_ptr(),
                self.inner.as_ptr(),
                uri.as_ptr(),
            )
        })?;
        Ok(AsyncDestination { _subscription: self.clone(), inner: inner.into() })
    }

    pub fn remove_destination(self: &Arc<Self>, uri: &str) -> Result<AsyncDestination> {
        let uri = CString::new(uri.as_bytes())?;
        let mut inner = ptr::null_mut();
        aeron_result(unsafe {
            sys::aeron_subscription_async_remove_destination(
                &mut inner,
                self.client.inner.as_ptr(),
                self.inner.as_ptr(),
                uri.as_ptr(),
            )
        })?;
        Ok(AsyncDestination { _subscription: self.clone(), inner: inner.into() })
    }

    pub fn channel_status(&self) -> ChannelStatus {
        match unsafe { sys::aeron_subscription_channel_status(self.inner.as_ptr()) } {
            1 => ChannelStatus::Active,
            -1 => ChannelStatus::Errored,
            v => ChannelStatus::Other(v),
        }
    }

    pub fn is_connected(&self) -> bool {
        unsafe { sys::aeron_subscription_is_connected(self.inner.as_ptr()) }
    }

    pub fn is_closed(&self) -> bool {
        unsafe { sys::aeron_subscription_is_closed(self.inner.as_ptr()) }
    }

    pub fn poll<F>(&self, handler: F, fragment_limit: usize)
    where
        F: for<'a> FnMut(&'a [u8], Header),
    {
        let mut closure = handler;
        unsafe {
            sys::aeron_subscription_poll(
                self.inner.as_ptr(),
                Some(fragment_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
                fragment_limit,
            )
        };
    }

    pub fn controlled_poll<F>(&self, handler: F, fragment_limit: usize)
    where
        F: for<'a> FnMut(&'a [u8], sys::aeron_header_values_t) -> HandlerAction,
    {
        let mut closure = handler;
        unsafe {
            sys::aeron_subscription_controlled_poll(
                self.inner.as_ptr(),
                Some(controlled_fragment_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
                fragment_limit,
            )
        };
    }

    pub fn block_poll<F>(&self, handler: F, block_length_limit: usize)
    where
        F: for<'a> FnMut(&'a [u8], SessionId, TermId),
    {
        let mut closure = handler;
        unsafe {
            sys::aeron_subscription_block_poll(
                self.inner.as_ptr(),
                Some(block_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
                block_length_limit,
            )
        };
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        aeron_result(unsafe {
            sys::aeron_subscription_close(self.inner.as_ptr(), None, ptr::null_mut())
        })
        .ok();
        // TODO: err
    }
}

pub struct AsyncDestination {
    _subscription: Arc<Subscription>,
    inner: SendSyncPtr<sys::aeron_async_destination_t>,
}

impl AsyncDestination {
    pub fn poll(&self) -> Result<bool> {
        let res = unsafe { sys::aeron_subscription_async_destination_poll(self.inner.as_ptr()) };
        if res >= 0 {
            Ok(res != 0)
        } else {
            Err(aeron_error(res))
        }
    }
}

unsafe extern "C" fn fragment_handler_trampoline<F>(
    clientd: *mut ffi::c_void,
    fragment: *const u8,
    fragment_length: usize,
    header: *mut sys::aeron_header_t,
) where
    F: for<'a> FnMut(&'a [u8], Header),
{
    let mut values: MaybeUninit<sys::aeron_header_values_t> = MaybeUninit::uninit();
    sys::aeron_header_values(header, values.as_mut_ptr()); // TODO: err
    let closure = &mut *(clientd as *mut F);
    let fragment = slice::from_raw_parts(fragment, fragment_length);
    closure(fragment, Header(values.assume_init()));
}

#[repr(u32)]
pub enum HandlerAction {
    Continue = sys::aeron_controlled_fragment_handler_action_en_AERON_ACTION_CONTINUE,
    Break = sys::aeron_controlled_fragment_handler_action_en_AERON_ACTION_BREAK,
    Abort = sys::aeron_controlled_fragment_handler_action_en_AERON_ACTION_ABORT,
    Commit = sys::aeron_controlled_fragment_handler_action_en_AERON_ACTION_COMMIT,
}

// TODO: replace aeron_header_values_t with custom type
unsafe extern "C" fn controlled_fragment_handler_trampoline<F>(
    clientd: *mut ffi::c_void,
    buffer: *const u8,
    length: usize,
    header: *mut sys::aeron_header_t,
) -> u32
where
    F: for<'a> FnMut(&'a [u8], sys::aeron_header_values_t) -> HandlerAction,
{
    let mut values: MaybeUninit<sys::aeron_header_values_t> = MaybeUninit::uninit();
    sys::aeron_header_values(header, values.as_mut_ptr()); // TODO: err
    let closure = &mut *(clientd as *mut F);
    let fragment = slice::from_raw_parts(buffer, length);
    closure(fragment, values.assume_init()) as u32
}

unsafe extern "C" fn block_handler_trampoline<F>(
    clientd: *mut ffi::c_void,
    buffer: *const u8,
    length: usize,
    session_id: i32,
    term_id: i32,
) where
    F: for<'a> FnMut(&'a [u8], SessionId, TermId),
{
    let closure = &mut *(clientd as *mut F);
    let fragment = slice::from_raw_parts(buffer, length);
    closure(fragment, SessionId(session_id), TermId(term_id));
}

#[must_use = "future must be polled"]
pub struct AddSubscription {
    client: Arc<Aeron>,
    state: AddSubscriptionState,
}

enum AddSubscriptionState {
    Unstarted { uri: String, stream_id: StreamId },
    Polling { inner: SendSyncPtr<sys::aeron_async_add_subscription_t> },
}

impl AddSubscription {
    pub(crate) fn new(client: &Arc<Aeron>, uri: &str, stream_id: StreamId) -> Result<Self> {
        Ok(AddSubscription {
            client: client.clone(),
            state: AddSubscriptionState::Unstarted { uri: uri.to_string(), stream_id },
        })
    }
}

impl Future for AddSubscription {
    type Output = Result<Subscription>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let mut self_mut = self.as_mut();
        match &self_mut.state {
            AddSubscriptionState::Unstarted { uri, stream_id } => {
                let s = CString::new(uri.as_bytes())?;

                let mut inner = ptr::null_mut();
                aeron_result(unsafe {
                    sys::aeron_async_add_subscription(
                        &mut inner,
                        self_mut.client.inner.as_ptr(),
                        s.as_ptr(),
                        stream_id.0,
                        None, // TODO: on_available_image_handler
                        ptr::null_mut(),
                        None, // TODO: on_unavailable_image_handler
                        ptr::null_mut(),
                    )
                })?;
                self_mut.state = AddSubscriptionState::Polling { inner: inner.into() };
                ctx.waker().wake_by_ref();
                Poll::Pending
            }
            AddSubscriptionState::Polling { inner } => {
                let mut subscription = ptr::null_mut();
                match unsafe {
                    sys::aeron_async_add_subscription_poll(&mut subscription, inner.as_ptr())
                } {
                    0 => {
                        ctx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    1 => {
                        debug_assert_ne!(subscription, ptr::null_mut());
                        Poll::Ready(Ok(Subscription::new(&self_mut.client, subscription)))
                    }
                    e => Poll::Ready(Err(aeron_error(e))),
                }
            }
        }
    }
}
