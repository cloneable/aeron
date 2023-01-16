use crate::{
    client::{Aeron, StreamId},
    error::{aeron_error, aeron_result, Result},
};
use aeron_client_sys::{
    aeron_async_add_subscription, aeron_async_add_subscription_poll,
    aeron_async_add_subscription_t, aeron_subscription_close, aeron_subscription_t,
};
use std::{
    ffi::CString,
    {future::Future, pin::Pin, sync::Arc, task::Poll},
    {ptr, task},
};

pub struct Subscription {
    _client: Arc<Aeron>,
    inner: *mut aeron_subscription_t,
}

impl Subscription {
    fn new(client: Arc<Aeron>, inner: *mut aeron_subscription_t) -> Self {
        debug_assert_ne!(inner, ptr::null_mut());
        Subscription {
            _client: client,
            inner,
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        aeron_result(unsafe { aeron_subscription_close(self.inner, None, ptr::null_mut()) }).ok();
    }
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
