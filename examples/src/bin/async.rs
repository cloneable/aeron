use aeron::{
    client::{Aeron, StreamId},
    context::Context,
};

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    let ctx = Context::new()?;
    let client = Aeron::connect(ctx)?;

    println!("Trying to add publication...");

    let add_publication = client
        .clone()
        .add_publication(&"aeron:ipc".to_owned(), StreamId(1001))?;

    println!("Awaiting publication...");

    let mut _publication = add_publication.await?;

    println!("Yay!");

    Ok(())
}
