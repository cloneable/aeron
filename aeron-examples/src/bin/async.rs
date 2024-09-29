use aeron::{client::Aeron, context::Context, publication::OfferResult, Header, StreamId};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

const DEFAULT_CHANNEL: &str = "aeron:udp?endpoint=localhost:20121";
const DEFAULT_STREAM_ID: StreamId = StreamId(1001);

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    let mut ctx = Context::new()?;

    ctx.set_error_handler(|e| {
        println!("P: ERR{}: {}", e.code, e.message);
    });
    ctx.set_on_new_publication(|p| {
        println!(
            "P: New publication: channel={} stream_id={:?} session_id={:?} correlation_id={:?}",
            p.channel, p.stream_id, p.session_id, p.correlation_id
        );
    });
    ctx.set_on_new_subscription(|s| {
        println!(
            "P: New subscription: channel={} stream_id={:?} correlation_id={:?}",
            s.channel, s.stream_id, s.correlation_id
        );
    });
    let client_pub = Aeron::connect(ctx)?;

    let mut publication = client_pub.add_publication(DEFAULT_CHANNEL, DEFAULT_STREAM_ID)?.await?;

    let mut ctx = Context::new()?;
    ctx.set_error_handler(|e| {
        println!("S: ERR{}: {}", e.code, e.message);
    });
    ctx.set_on_new_publication(|p| {
        println!(
            "S: New publication: channel={} stream_id={:?} session_id={:?} correlation_id={:?}",
            p.channel, p.stream_id, p.session_id, p.correlation_id
        );
    });
    ctx.set_on_new_subscription(|s| {
        println!(
            "S: New subscription: channel={} stream_id={:?} correlation_id={:?}",
            s.channel, s.stream_id, s.correlation_id
        );
    });
    let client_sub = Aeron::connect(ctx)?;

    let subscription =
        client_sub.add_subscription(DEFAULT_CHANNEL, DEFAULT_STREAM_ID).unwrap().await.unwrap();

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
