use crate::{
    context::Context,
    error::{aeron_result, Result},
    publication::AddPublication,
    subscription::AddSubscription,
    SendSyncPtr, StreamId,
};
use aeron_client_sys::{aeron_close, aeron_init, aeron_start, aeron_t};
use std::{ptr, sync::Arc};

pub struct Aeron {
    pub context: Context,
    pub(crate) inner: SendSyncPtr<aeron_t>,
}

impl Aeron {
    pub fn connect(context: Context) -> Result<Arc<Self>> {
        let mut inner = ptr::null_mut();
        aeron_result(unsafe { aeron_init(&mut inner, context.inner.as_ptr()) })?;
        aeron_result(unsafe { aeron_start(inner) })?;
        Ok(Arc::new(Aeron { context, inner: inner.into() }))
    }

    pub fn add_publication(
        self: &Arc<Self>,
        uri: &str,
        stream_id: StreamId,
    ) -> Result<AddPublication> {
        AddPublication::new(self, uri, stream_id)
    }

    pub fn add_subscription(
        self: &Arc<Self>,
        uri: &str,
        stream_id: StreamId,
    ) -> Result<AddSubscription> {
        AddSubscription::new(self, uri, stream_id)
    }
}

impl Drop for Aeron {
    fn drop(&mut self) {
        aeron_result(unsafe { aeron_close(self.inner.as_ptr()) }).ok(); // TODO: err
    }
}
