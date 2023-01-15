use crate::{
    client::{Aeron, Position, StreamId},
    error::{aeron_error, aeron_result, Result},
};
use aeron_client_sys::{
    aeron_async_add_publication, aeron_async_add_publication_poll, aeron_async_add_publication_t,
    aeron_publication_close, aeron_publication_offer, aeron_publication_t,
};
use std::{
    ffi::CString,
    {future::Future, pin::Pin, sync::Arc, task::Poll},
    {ptr, task},
};

pub struct Publication {
    _client: Arc<Aeron>,
    inner: *mut aeron_publication_t,
}

impl Publication {
    fn new(client: Arc<Aeron>, inner: *mut aeron_publication_t) -> Self {
        debug_assert_ne!(inner, ptr::null_mut());
        Publication {
            _client: client,
            inner,
        }
    }

    pub fn offer(&mut self, data: &Vec<u8>) -> Result<OfferResult> {
        let res = unsafe {
            aeron_publication_offer(self.inner, data.as_ptr(), data.len(), None, ptr::null_mut())
        };
        if res >= 0 {
            return Ok(OfferResult::Ok(Position(res)));
        }
        match res {
            -1 => Ok(OfferResult::NotConnected),
            -2 => Ok(OfferResult::BackPressured),
            -3 => Ok(OfferResult::AdminAction),
            _ => Err(aeron_error(res as i32)),
        }
    }
}

impl Drop for Publication {
    fn drop(&mut self) {
        aeron_result(unsafe { aeron_publication_close(self.inner, None, ptr::null_mut()) }).ok();
    }
}

pub enum OfferResult {
    Ok(Position),
    BackPressured,
    NotConnected,
    AdminAction,
}

#[must_use = "future must be polled"]
pub struct AddPublication {
    client: Arc<Aeron>,
    state: AddPublicationState,
}

enum AddPublicationState {
    Unstarted {
        uri: String,
        stream_id: StreamId,
    },
    Polling {
        inner: *mut aeron_async_add_publication_t,
    },
}

impl AddPublication {
    pub(crate) fn new(client: Arc<Aeron>, uri: &String, stream_id: StreamId) -> Result<Self> {
        Ok(AddPublication {
            client,
            state: AddPublicationState::Unstarted {
                uri: uri.clone(),
                stream_id,
            },
        })
    }
}

impl Future for AddPublication {
    type Output = Result<Publication>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let mut self_mut = self.as_mut();
        match &self_mut.state {
            AddPublicationState::Unstarted { uri, stream_id } => {
                let s = CString::new(uri.as_bytes())?;

                let mut add_publication = ptr::null_mut();
                aeron_result(unsafe {
                    aeron_async_add_publication(
                        &mut add_publication,
                        self_mut.client.inner,
                        s.as_ptr(),
                        stream_id.0,
                    )
                })?;
                debug_assert_ne!(add_publication, ptr::null_mut());

                self_mut.state = AddPublicationState::Polling {
                    inner: add_publication,
                };
                ctx.waker().wake_by_ref();
                Poll::Pending
            }
            AddPublicationState::Polling { inner } => {
                let mut publication = ptr::null_mut();
                match unsafe { aeron_async_add_publication_poll(&mut publication, *inner) } {
                    0 => {
                        ctx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    1 => {
                        debug_assert_ne!(publication, ptr::null_mut());
                        Poll::Ready(Ok(Publication::new(self_mut.client.clone(), publication)))
                    }
                    e => Poll::Ready(Err(aeron_error(e))),
                }
            }
        }
    }
}
