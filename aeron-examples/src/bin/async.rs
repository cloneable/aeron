use aeron::{client::Aeron, context::Context, publication::OfferResult, Header, StreamId};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[global_allocator]
static ALLOCATOR: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

const DEFAULT_CHANNEL: &str = "aeron:udp?endpoint=localhost:20121";
const DEFAULT_STREAM_ID: StreamId = StreamId(1001);

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    let mut ctx = Context::new()?;
    ctx.set_error_handler(|code, msg| {
        println!("P: ERR{code}: {msg}");
    });
    ctx.set_on_new_publication(|channel, stream_id, session_id, correlation_id| {
        println!("P: New publication: channel={channel} stream_id={stream_id:?} session_id={session_id:?} correlation_id={correlation_id:?}");
    });
    ctx.set_on_new_subscription(|channel, stream_id, correlation_id| {
        println!("P: New subscription: channel={channel} stream_id={stream_id:?} correlation_id={correlation_id:?}");
    });
    let client_p = Aeron::connect(ctx)?;

    let mut publication = client_p.add_publication(DEFAULT_CHANNEL, DEFAULT_STREAM_ID)?.await?;

    let mut ctx = Context::new()?;
    ctx.set_error_handler(|code, msg| {
        println!("S: ERR{code}: {msg}");
    });
    ctx.set_on_new_publication(|channel, stream_id, session_id, correlation_id| {
        println!("S: New publication: channel={channel} stream_id={stream_id:?} session_id={session_id:?} correlation_id={correlation_id:?}");
    });
    ctx.set_on_new_subscription(|channel, stream_id, correlation_id| {
        println!("S: New subscription: channel={channel} stream_id={stream_id:?} correlation_id={correlation_id:?}");
    });
    let client_s = Aeron::connect(ctx)?;

    let subscription =
        client_s.add_subscription(DEFAULT_CHANNEL, DEFAULT_STREAM_ID).unwrap().await.unwrap();

    let handle = tokio::spawn(async move {
        let stop = AtomicBool::new(false);
        let handler = |data: &[u8], header: Header| {
            let text = String::from_utf8_lossy(data);
            println!(
                "S: Message from session {sess_id:?} ({len} bytes) <<{text}>>",
                sess_id = header.session_id(),
                len = data.len(),
            );
            if text == "stop" {
                stop.store(true, Ordering::Release);
            }
        };

        while !stop.load(Ordering::Acquire) {
            subscription.poll(handler, 1);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    let buf = vec![42u8];
    for _ in 0..10 {
        match publication.offer(&buf)? {
            OfferResult::Ok(position) => {
                println!("P: SENT {position:?}");
            }
            OfferResult::NotConnected => {
                println!("P: no subscriber connnected. retying.");
            }
            OfferResult::BackPressured => {
                println!("P: back pressured. retying.");
            }
            OfferResult::AdminAction => {
                println!("P: admin action. retying.");
            }
        };
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let (mut buf, _) = publication.try_claim(26)?;
    for i in 0..buf.data().len() {
        buf.data()[i] = b'a' + i as u8;
    }
    buf.commit()?;

    publication.offer(&Vec::from("stop".as_bytes()))?;

    handle.await?;

    Ok(())
}
