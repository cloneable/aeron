use crate::{
    client::Aeron,
    error::{aeron_error, aeron_result, Result},
    ChannelStatus, Position, SendSyncPtr, StreamId,
};
use aeron_client_sys::{
    aeron_async_add_publication, aeron_async_add_publication_poll, aeron_async_add_publication_t,
    aeron_async_destination_t, aeron_buffer_claim_abort, aeron_buffer_claim_commit,
    aeron_buffer_claim_stct, aeron_publication_async_add_destination,
    aeron_publication_async_destination_poll, aeron_publication_async_remove_destination,
    aeron_publication_channel_status, aeron_publication_close, aeron_publication_is_closed,
    aeron_publication_is_connected, aeron_publication_offer, aeron_publication_t,
    aeron_publication_try_claim,
};
use std::{
    ffi,
    ffi::CString,
    mem::MaybeUninit,
    slice,
    {future::Future, pin::Pin, sync::Arc, task::Poll},
    {ptr, task},
};

pub struct Publication {
    client: Arc<Aeron>,
    inner: SendSyncPtr<aeron_publication_t>,
}

impl Publication {
    fn new(client: Arc<Aeron>, inner: *mut aeron_publication_t) -> Self {
        Publication { client, inner: inner.into() }
    }

    pub fn channel_status(&self) -> ChannelStatus {
        match unsafe { aeron_publication_channel_status(self.inner.as_ptr()) } {
            1 => ChannelStatus::Active,
            -1 => ChannelStatus::Errored,
            v => ChannelStatus::Other(v),
        }
    }

    pub fn is_connected(&self) -> bool {
        unsafe { aeron_publication_is_connected(self.inner.as_ptr()) }
    }

    pub fn is_closed(&self) -> bool {
        unsafe { aeron_publication_is_closed(self.inner.as_ptr()) }
    }

    pub fn offer(&mut self, data: &[u8]) -> Result<OfferResult> {
        let res = unsafe {
            aeron_publication_offer(
                self.inner.as_ptr(),
                data.as_ptr(),
                data.len(),
                None,
                ptr::null_mut(),
            )
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

    pub fn offer_with_reserved_value_supplier<F>(
        &mut self,
        data: &[u8],
        reserved_value_supplier: F,
    ) -> Result<OfferResult>
    where
        F: for<'a> FnMut(&'a mut [u8]) -> i64,
    {
        let mut closure = reserved_value_supplier;
        let res = unsafe {
            aeron_publication_offer(
                self.inner.as_ptr(),
                data.as_ptr(),
                data.len(),
                Some(reserved_value_supplier_trampoline::<F>),
                &mut closure as *mut _ as *mut ffi::c_void,
            )
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

    pub fn try_claim(&self, length: usize) -> Result<(BufferClaim, Position)> {
        let mut buffer_claim: MaybeUninit<aeron_buffer_claim_stct> = MaybeUninit::uninit();
        let ret = unsafe {
            aeron_publication_try_claim(self.inner.as_ptr(), length, buffer_claim.as_mut_ptr())
        };
        if ret >= 0 {
            Ok((BufferClaim { inner: unsafe { buffer_claim.assume_init() } }, Position(ret)))
        } else {
            Err(aeron_error(ret as i32))
        }
    }

    pub fn add_destination(self: &Arc<Publication>, uri: &str) -> Result<AsyncDestination> {
        let uri = CString::new(uri.as_bytes())?;
        let mut inner = ptr::null_mut();
        aeron_result(unsafe {
            aeron_publication_async_add_destination(
                &mut inner,
                self.client.inner.as_ptr(),
                self.inner.as_ptr(),
                uri.as_ptr(),
            )
        })?;
        Ok(AsyncDestination { _publication: self.clone(), inner: inner.into() })
    }

    pub fn remove_destination(self: &Arc<Publication>, uri: &str) -> Result<AsyncDestination> {
        let uri = CString::new(uri.as_bytes())?;
        let mut inner = ptr::null_mut();
        aeron_result(unsafe {
            aeron_publication_async_remove_destination(
                &mut inner,
                self.client.inner.as_ptr(),
                self.inner.as_ptr(),
                uri.as_ptr(),
            )
        })?;
        Ok(AsyncDestination { _publication: self.clone(), inner: inner.into() })
    }
}

impl Drop for Publication {
    fn drop(&mut self) {
        aeron_result(unsafe {
            aeron_publication_close(self.inner.as_ptr(), None, ptr::null_mut())
        })
        .ok();
    }
}

pub enum OfferResult {
    Ok(Position),
    BackPressured,
    NotConnected,
    AdminAction,
}

pub struct BufferClaim {
    inner: aeron_buffer_claim_stct,
}

impl BufferClaim {
    pub fn data(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.inner.data, self.inner.length) }
    }

    pub fn commit(mut self) -> Result<()> {
        aeron_result(unsafe { aeron_buffer_claim_commit(&mut self.inner) })
    }

    pub fn abort(mut self) -> Result<()> {
        aeron_result(unsafe { aeron_buffer_claim_abort(&mut self.inner) })
    }
}

unsafe extern "C" fn reserved_value_supplier_trampoline<F>(
    clientd: *mut ffi::c_void,
    buffer: *mut u8,
    frame_length: usize,
) -> i64
where
    F: for<'a> FnMut(&'a mut [u8]) -> i64,
{
    let closure = &mut *(clientd as *mut F);
    let frame = slice::from_raw_parts_mut(buffer, frame_length);
    closure(frame)
}

#[must_use = "future must be polled"]
pub struct AddPublication {
    client: Arc<Aeron>,
    state: AddPublicationState,
}

enum AddPublicationState {
    Unstarted { uri: String, stream_id: StreamId },
    Polling { inner: SendSyncPtr<aeron_async_add_publication_t> },
}

impl AddPublication {
    pub(crate) fn new(client: Arc<Aeron>, uri: &String, stream_id: StreamId) -> Result<Self> {
        Ok(AddPublication {
            client,
            state: AddPublicationState::Unstarted { uri: uri.clone(), stream_id },
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

                let mut inner = ptr::null_mut();
                aeron_result(unsafe {
                    aeron_async_add_publication(
                        &mut inner,
                        self_mut.client.inner.as_ptr(),
                        s.as_ptr(),
                        stream_id.0,
                    )
                })?;
                debug_assert_ne!(inner, ptr::null_mut());

                self_mut.state = AddPublicationState::Polling { inner: inner.into() };
                ctx.waker().wake_by_ref();
                Poll::Pending
            }
            AddPublicationState::Polling { inner } => {
                let mut publication = ptr::null_mut();
                match unsafe { aeron_async_add_publication_poll(&mut publication, inner.as_ptr()) }
                {
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

pub struct AsyncDestination {
    _publication: Arc<Publication>,
    inner: SendSyncPtr<aeron_async_destination_t>,
}

impl AsyncDestination {
    pub fn poll(&self) -> Result<bool> {
        let res = unsafe { aeron_publication_async_destination_poll(self.inner.as_ptr()) };
        if res >= 0 {
            Ok(res != 0)
        } else {
            Err(aeron_error(res))
        }
    }
}
