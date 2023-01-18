pub mod client;
pub mod context;
pub mod error;
pub mod publication;
pub mod subscription;

pub struct SessionId(i32);

pub struct TermId(i32);

#[repr(i64)]
pub enum ChannelStatus {
    Active = 1,
    Errored = -1,
    Other(i64),
}
