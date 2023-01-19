use crate::{
    client::Aeron,
    error::{aeron_error, aeron_result, Result},
    ChannelStatus, SessionId, StreamId, TermId,
};
use aeron_client_sys::{
    aeron_async_add_subscription, aeron_async_add_subscription_poll,
    aeron_async_add_subscription_t, aeron_async_destination_t,
    aeron_controlled_fragment_handler_action_en_AERON_ACTION_ABORT,
    aeron_controlled_fragment_handler_action_en_AERON_ACTION_BREAK,
    aeron_controlled_fragment_handler_action_en_AERON_ACTION_COMMIT,
    aeron_controlled_fragment_handler_action_en_AERON_ACTION_CONTINUE, aeron_header_t,
    aeron_header_values, aeron_header_values_t, aeron_subscription_async_add_destination,
    aeron_subscription_async_destination_poll, aeron_subscription_async_remove_destination,
    aeron_subscription_block_poll, aeron_subscription_channel_status, aeron_subscription_close,
    aeron_subscription_controlled_poll, aeron_subscription_is_closed,
    aeron_subscription_is_connected, aeron_subscription_poll, aeron_subscription_t,
};
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
    inner: *mut aeron_subscription_t,
}

unsafe impl Send for Subscription {
    // TODO: verify that the C client doesn't use TLS
}

impl Subscription {
    fn new(client: Arc<Aeron>, inner: *mut aeron_subscription_t) -> Self {
        debug_assert_ne!(inner, ptr::null_mut());
        Subscription { client, inner }
    }

    pub fn add_destination(self: &Arc<Subscription>, uri: &str) -> Result<AsyncDestination> {
        let uri = CString::new(uri.as_bytes())?;
        let mut inner = ptr::null_mut();
        aeron_result(unsafe {
            aeron_subscription_async_add_destination(
                &mut inner,
                self.client.inner,
                self.inner,
                uri.as_ptr(),
            )
        })?;
        Ok(AsyncDestination {
            _subscription: self.clone(),
            inner,
        })
    }

    pub fn remove_destination(self: &Arc<Subscription>, uri: &str) -> Result<AsyncDestination> {
        let uri = CString::new(uri.as_bytes())?;
        let mut inner = ptr::null_mut();
        aeron_result(unsafe {
            aeron_subscription_async_remove_destination(
                &mut inner,
                self.client.inner,
                self.inner,
                uri.as_ptr(),
            )
        })?;
        Ok(AsyncDestination {
            _subscription: self.clone(),
            inner,
        })
    }

    pub fn channel_status(&self) -> ChannelStatus {
        match unsafe { aeron_subscription_channel_status(self.inner) } {
            1 => ChannelStatus::Active,
            -1 => ChannelStatus::Errored,
            v => ChannelStatus::Other(v),
        }
    }

    pub fn is_connected(&self) -> bool {
        unsafe { aeron_subscription_is_connected(self.inner) }
    }

    pub fn is_closed(&self) -> bool {
        unsafe { aeron_subscription_is_closed(self.inner) }
    }

    pub fn poll<'a, F: FragmentHandler<'a>>(&self, handler: F, fragment_limit: usize) {
        let mut closure = handler;
        unsafe {
            aeron_subscription_poll(
                self.inner,
                Some(fragment_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
                fragment_limit,
            )
        };
    }

    pub fn controlled_poll<'a, F: ControlledFragmentHandler<'a>>(
        &self,
        handler: F,
        fragment_limit: usize,
    ) {
        let mut closure = handler;
        unsafe {
            aeron_subscription_controlled_poll(
                self.inner,
                Some(controlled_fragment_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
                fragment_limit,
            )
        };
    }

    pub fn block_poll<'a, F: BlockHandler<'a>>(&self, handler: F, block_length_limit: usize) {
        let mut closure = handler;
        unsafe {
            aeron_subscription_block_poll(
                self.inner,
                Some(block_handler_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
                block_length_limit,
            )
        };
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        aeron_result(unsafe { aeron_subscription_close(self.inner, None, ptr::null_mut()) }).ok();
        // TODO: err
    }
}

pub struct AsyncDestination {
    _subscription: Arc<Subscription>,
    inner: *mut aeron_async_destination_t,
}

impl AsyncDestination {
    pub fn poll(&self) -> Result<bool> {
        let res = unsafe { aeron_subscription_async_destination_poll(self.inner) };
        if res >= 0 {
            Ok(res != 0)
        } else {
            Err(aeron_error(res))
        }
    }
}

pub trait FragmentHandler<'a>: FnMut(&'a [u8], aeron_header_values_t) {}

impl<'a, F> FragmentHandler<'a> for F where F: FnMut(&'a [u8], aeron_header_values_t) {}

unsafe extern "C" fn fragment_handler_trampoline<'a, F: FragmentHandler<'a>>(
    clientd: *mut ffi::c_void,
    fragment: *const u8,
    fragment_length: usize,
    header: *mut aeron_header_t,
) {
    let mut values: MaybeUninit<aeron_header_values_t> = MaybeUninit::uninit();
    aeron_header_values(header, values.as_mut_ptr()); // TODO: err
    let closure = &mut *(clientd as *mut F);
    let fragment = slice::from_raw_parts(fragment, fragment_length);
    closure(fragment, values.assume_init());
}

#[repr(u32)]
pub enum HandlerAction {
    Continue = aeron_controlled_fragment_handler_action_en_AERON_ACTION_CONTINUE,
    Break = aeron_controlled_fragment_handler_action_en_AERON_ACTION_BREAK,
    Abort = aeron_controlled_fragment_handler_action_en_AERON_ACTION_ABORT,
    Commit = aeron_controlled_fragment_handler_action_en_AERON_ACTION_COMMIT,
}

// TODO: replace aeron_header_values_t with custom type
pub trait ControlledFragmentHandler<'a>:
    FnMut(&'a [u8], aeron_header_values_t) -> HandlerAction
{
}

impl<'a, F> ControlledFragmentHandler<'a> for F where
    F: FnMut(&'a [u8], aeron_header_values_t) -> HandlerAction
{
}

unsafe extern "C" fn controlled_fragment_handler_trampoline<
    'a,
    F: ControlledFragmentHandler<'a>,
>(
    clientd: *mut ffi::c_void,
    buffer: *const u8,
    length: usize,
    header: *mut aeron_header_t,
) -> u32 {
    let mut values: MaybeUninit<aeron_header_values_t> = MaybeUninit::uninit();
    aeron_header_values(header, values.as_mut_ptr()); // TODO: err
    let closure = &mut *(clientd as *mut F);
    let fragment = slice::from_raw_parts(buffer, length);
    closure(fragment, values.assume_init()) as u32
}

pub trait BlockHandler<'a>: FnMut(&'a [u8], SessionId, TermId) {}

impl<'a, F> BlockHandler<'a> for F where F: FnMut(&'a [u8], SessionId, TermId) {}

unsafe extern "C" fn block_handler_trampoline<'a, F: BlockHandler<'a>>(
    clientd: *mut ffi::c_void,
    buffer: *const u8,
    length: usize,
    session_id: i32,
    term_id: i32,
) {
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
    Unstarted {
        uri: String,
        stream_id: StreamId,
    },
    Polling {
        inner: *mut aeron_async_add_subscription_t,
    },
}

impl AddSubscription {
    pub(crate) fn new(client: Arc<Aeron>, uri: &String, stream_id: StreamId) -> Result<Self> {
        Ok(AddSubscription {
            client,
            state: AddSubscriptionState::Unstarted {
                uri: uri.clone(),
                stream_id,
            },
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

                let mut add_subscription = ptr::null_mut();
                aeron_result(unsafe {
                    aeron_async_add_subscription(
                        &mut add_subscription,
                        self_mut.client.inner,
                        s.as_ptr(),
                        stream_id.0,
                        None, // TODO: on_available_image_handler
                        ptr::null_mut(),
                        None, // TODO: on_unavailable_image_handler
                        ptr::null_mut(),
                    )
                })?;
                debug_assert_ne!(add_subscription, ptr::null_mut());

                self_mut.state = AddSubscriptionState::Polling {
                    inner: add_subscription,
                };
                ctx.waker().wake_by_ref();
                Poll::Pending
            }
            AddSubscriptionState::Polling { inner } => {
                let mut subscription = ptr::null_mut();
                match unsafe { aeron_async_add_subscription_poll(&mut subscription, *inner) } {
                    0 => {
                        ctx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    1 => {
                        debug_assert_ne!(subscription, ptr::null_mut());
                        Poll::Ready(Ok(Subscription::new(self_mut.client.clone(), subscription)))
                    }
                    e => Poll::Ready(Err(aeron_error(e))),
                }
            }
        }
    }
}
