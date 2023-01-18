use aeron::{
    client::{Aeron, StreamId},
    context::Context,
    publication::OfferResult,
};
use aeron_client_sys::aeron_header_values_t;
use std::time::Duration;

const DEFAULT_CHANNEL: &str = "aeron:udp?endpoint=localhost:20121";
const DEFAULT_STREAM_ID: StreamId = StreamId(1001);

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    let ctx = Context::new()?;
    let client = Aeron::connect(ctx)?;

    let mut publication = client
        .clone()
        .add_publication(&DEFAULT_CHANNEL.to_owned(), DEFAULT_STREAM_ID)?
        .await?;

    let subscription = client
        .add_subscription(&DEFAULT_CHANNEL.to_owned(), DEFAULT_STREAM_ID)
        .unwrap()
        .await
        .unwrap();

    tokio::spawn(async move {
        let handler = |data: &[u8], header: aeron_header_values_t| {
            let text = String::from_utf8_lossy(data);
            println!(
                "Message from session {sess_id} ({len} bytes) <<{text}>>",
                sess_id = header.frame.session_id,
                len = data.len(),
            );
        };

        loop {
            subscription.poll(handler, 100);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    let buf = vec![42u8];
    loop {
        match publication.offer(&buf)? {
            OfferResult::Ok(position) => {
                println!("SENT {position:?}");
            }
            OfferResult::NotConnected => {
                println!("no subscriber connnected. retying.");
            }
            OfferResult::BackPressured => {
                println!("back pressured. retying.");
            }
            OfferResult::AdminAction => {
                println!("admin action. retying.");
            }
        };
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
