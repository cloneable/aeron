use aeron::{
    client::{Aeron, StreamId},
    context::Context,
};

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    let ctx = Context::new()?;
    let client = Aeron::connect(ctx)?;

    let mut _publication = client
        .add_publication(&"aeron:ipc".to_owned(), StreamId(1001))?
        .await?;

    println!("Yay!");

    Ok(())
}
