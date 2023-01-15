use aeron::{
    client::{Aeron, StreamId},
    context::Context,
};

const DEFAULT_CHANNEL: &str = "aeron:udp?endpoint=localhost:20121";
const DEFAULT_STREAM_ID: StreamId = StreamId(1001);

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    let ctx = Context::new()?;
    let client = Aeron::connect(ctx)?;

    let mut publication = client
        .add_publication(&DEFAULT_CHANNEL.to_owned(), DEFAULT_STREAM_ID)?
        .await?;

    println!("Yay!");

    Ok(())
}
