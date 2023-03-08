use std::{cmp::Ordering, error::Error};

use futures::{future::Either, SinkExt, StreamExt};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use tokio::{select, sync::broadcast::Sender};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    iter_utils::OrderedChainExt,
    service::{Level as SummaryLevel, Summary},
    venue_protocols::*,
};

#[derive(Default)]
struct Book {
    bids: Vec<Level>,
    asks: Vec<Level>,
}

fn aggregate_levels(
    bitstamp_levels: &[Level],
    binance_levels: &[Level],
    cmp: Ordering,
) -> Vec<SummaryLevel> {
    let make_summary_level = |exchange: &str, price: &Decimal, qty: &Decimal| SummaryLevel {
        exchange: exchange.to_string(),
        price: price.to_f64().unwrap(),
        amount: qty.to_f64().unwrap(),
    };

    bitstamp_levels
        .iter()
        .ordered_chain(binance_levels.iter(), cmp)
        .map(|level| match level {
            Either::Left((price, qty)) => make_summary_level("bitstamp", price, qty),
            Either::Right((price, qty)) => make_summary_level("binance", price, qty),
        })
        .take(10)
        .collect()
}

pub async fn aggregator_task(symbol: String, tx: Sender<Summary>) -> Result<(), Box<dyn Error>> {
    let (mut bitstamp_ws, _) = tokio_tungstenite::connect_async("wss://ws.bitstamp.net.").await.expect("Failed to connect to bitstamp websocket");

    let subscription_message = bitstamp::make_subscription_payload(&symbol);
    bitstamp_ws
        .send(Message::Text(subscription_message))
        .await?;

    let (mut binance_ws, _) = tokio_tungstenite::connect_async(format!(
        "wss://stream.binance.com:9443/ws/{symbol}@depth20@100ms"
    ))
    .await?;

    let mut bitstamp_book = Book::default();
    let mut binance_book = Book::default();

    loop {
        select! {
            result = bitstamp_ws.next() => {
                // println!("Bitstamp {result:?}");
                match result {

                    Some(Ok(Message::Text(payload))) => {
                        match serde_json::from_str::<bitstamp::FeedMessage>(&payload) {
                            Ok(bitstamp::FeedMessage::Data {data, ..}) => {
                                bitstamp_book.bids = data.bids;
                                bitstamp_book.asks = data.asks;

                                // The venue already provides the levels sorted, but lets sort anyway
                                bitstamp_book.bids.sort_by(|(px_a, _), (px_b, _) | px_b.cmp(px_a));
                                bitstamp_book.asks.sort_by_key(|(px, _)| *px);

                            }
                            Ok(bitstamp::FeedMessage::Error {message, ..}) => panic!("Failed to subscribe to symbol, {message}"),
                            Err(e) => panic!("Failed to parse bitstamp payload, {payload}, {e}"),
                            _ => {
                                continue;
                            }
                        }

                    },
                    Some(Err(e)) => panic!("Error from bitstamp socket, {e}"),
                    None => panic!("Disconnected from bitstamp"),
                    _ => {}
                }
            },

            result = binance_ws.next() => {
                // println!("Binance {result:?}");
                match result {
                    Some(Ok(Message::Text(payload))) => {
                        match serde_json::from_str::<binance::BookUpdate>(&payload) {
                            Ok(update) => {
                                binance_book.bids = update.bids;
                                binance_book.asks = update.asks;

                                // The venue already provides the levels sorted, but lets sort anyway
                                binance_book.bids.sort_by(|(px_a, _), (px_b, _) | px_b.cmp(px_a));
                                binance_book.asks.sort_by_key(|(px, _)| *px);
                            }
                            Err(e) => panic!("Failed to parse payload from binance, {payload}, {e}")
                        }

                    },
                    Some(Err(e)) => panic!("Error from binance socket, {e}"),
                    None => panic!("Disconnected from binance"),
                    _ => {
                        continue;
                    }
                }
            }
        }

        let bids = aggregate_levels(&bitstamp_book.bids, &binance_book.bids, Ordering::Greater);
        let asks = aggregate_levels(&bitstamp_book.asks, &binance_book.asks, Ordering::Less);

        if bids.is_empty() || asks.is_empty() {
            // We can't publish a spread since the book is one-sided.
            continue;
        }

        let spread = asks[0].price - bids[0].price;

        tx.send(Summary { bids, asks, spread })?;
    }


}
