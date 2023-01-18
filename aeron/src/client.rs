use crate::{
    context::Context,
    error::{aeron_result, Result},
    publication::AddPublication,
    subscription::AddSubscription,
};
use aeron_client_sys::{aeron_close, aeron_init, aeron_start, aeron_t};
use std::{ptr, sync::Arc};

#[derive(Copy, Clone, Debug)]
pub struct StreamId(pub i32);

#[derive(Copy, Clone, Debug)]
pub struct Position(pub i64);

pub struct Aeron {
    pub(crate) inner: *mut aeron_t,
    pub context: Context,
}

unsafe impl Send for Aeron {
    // TODO: verify that the C client doesn't use TLS
}

impl Aeron {
    pub fn connect(context: Context) -> Result<Arc<Self>> {
        let mut inner = ptr::null_mut();
        aeron_result(unsafe { aeron_init(&mut inner, context.inner) })?;
        debug_assert_ne!(inner, ptr::null_mut());
        aeron_result(unsafe { aeron_start(inner) })?;
        Ok(Arc::new(Aeron { inner, context }))
    }

    pub fn add_publication(
        self: Arc<Self>,
        uri: &String,
        stream_id: StreamId,
    ) -> Result<AddPublication> {
        AddPublication::new(self, uri, stream_id)
    }

    pub fn add_subscription(
        self: Arc<Self>,
        uri: &String,
        stream_id: StreamId,
    ) -> Result<AddSubscription> {
        AddSubscription::new(self, uri, stream_id)
    }
}

impl Drop for Aeron {
    fn drop(&mut self) {
        aeron_result(unsafe { aeron_close(self.inner) }).ok(); // TODO: err
    }
}
