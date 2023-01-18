pub mod client;
pub mod context;
pub mod error;
pub mod publication;
pub mod subscription;

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
