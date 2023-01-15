use aeron::{
    client::{Aeron, StreamId},
    context::Context,
    publication::OfferResult,
};
use std::time::Duration;

const DEFAULT_CHANNEL: &str = "aeron:udp?endpoint=localhost:20121";
const DEFAULT_STREAM_ID: StreamId = StreamId(1001);

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    let ctx = Context::new()?;
    let client = Aeron::connect(ctx)?;

    let mut publication = client
        .add_publication(&DEFAULT_CHANNEL.to_owned(), DEFAULT_STREAM_ID)?
        .await?;

    let buf = vec![42u8];
    loop {
        match publication.offer(&buf)? {
            OfferResult::Ok(_position) => break,
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

    println!("Yay!");

    Ok(())
}
