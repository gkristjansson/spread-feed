use std::{cmp::Ordering, error::Error};

use futures::{SinkExt, StreamExt};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use tokio::{select, sync::broadcast::Sender};
use tokio_tungstenite::tungstenite::Message;

use crate::service::{Level as SummaryLevel, Summary};

type Price = Decimal;
type Qty = Decimal;
type Level = (Price, Qty);

#[derive(Default)]
struct Book {
    bids: Vec<Level>,
    asks: Vec<Level>,
}

mod bitstamp {
    use serde::Deserialize;
    use serde_json::json;

    use crate::aggregator::{Price, Qty};

    #[derive(Deserialize, Debug)]
    pub struct BookUpdate {
        pub bids: Vec<(Price, Qty)>,
        pub asks: Vec<(Price, Qty)>,
    }

    #[derive(Deserialize, Debug)]
    #[serde(tag = "event", rename_all = "snake_case")]
    pub enum FeedMessage {
        #[serde(rename = "bts:subscription_succeeded")]
        SubscriptionSucceeded {
            channel: String,
        },

        #[serde(rename = "bts:error")]
        Error {
            code: u64,
            message: String,
        },

        Data {
            channel: String,
            data: BookUpdate,
        },
    }

    pub fn make_subscription_payload(symbol: &str) -> String {
        json!({
            "event": "bts:subscribe",
            "data": {
                "channel": format!("order_book_{symbol}")
            }
        })
        .to_string()
    }
}

mod binance {
    use serde::Deserialize;

    use crate::aggregator::{Price, Qty};

    #[derive(Deserialize, Debug)]
    pub struct BookUpdate {
        pub bids: Vec<(Price, Qty)>,
        pub asks: Vec<(Price, Qty)>,
    }
}

fn aggerate_levels<F>(
    bitstamp_levels: &[Level],
    binance_levels: &[Level],
    cmp: F,
) -> Vec<SummaryLevel>
where
    F: Fn(&Price, &Price) -> Ordering,
{
    let mut ret = vec![];
    let mut bitstamp_it = bitstamp_levels.iter().peekable();
    let mut binance_it = binance_levels.iter().peekable();

    let make_level = |exchange: &str, px: &Price, qty: &Qty| SummaryLevel {
        exchange: exchange.to_string(),
        price: px.to_f64().unwrap(),
        amount: qty.to_f64().unwrap(),
    };

    while ret.len() < 10 {
        match (bitstamp_it.peek(), binance_it.peek()) {
            (Some((px, qty)), None) => {
                ret.push(make_level("bitstamp", px, qty));
                bitstamp_it.next();
            }
            (None, Some((px, qty))) => {
                ret.push(make_level("binance", px, qty));
                binance_it.next();
            }
            (Some((bitstamp_px, bitstamp_qty)), Some((binance_px, binance_qty))) => {
                match cmp(bitstamp_px, binance_px) {
                    Ordering::Less => {
                        ret.push(make_level("bitstamp", bitstamp_px, bitstamp_qty));
                        bitstamp_it.next();
                    }
                    Ordering::Equal => {
                        if bitstamp_qty > binance_qty {
                            ret.push(make_level("bitstamp", bitstamp_px, bitstamp_qty));
                            bitstamp_it.next();
                        } else {
                            ret.push(make_level("binance", binance_px, binance_qty));
                            binance_it.next();
                        }
                    }
                    Ordering::Greater => {
                        ret.push(make_level("binance", binance_px, binance_qty));
                        binance_it.next();
                    }
                }
            }
            (None, None) => break,
        }
    }

    ret
}

pub async fn aggregator_task(symbol: String, tx: Sender<Summary>) -> Result<(), Box<dyn Error>> {
    let (mut bitstamp_ws, _) = tokio_tungstenite::connect_async("wss://ws.bitstamp.net.").await?;

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

        let bids = aggerate_levels(&bitstamp_book.bids, &binance_book.bids, |a, b| {
            a.cmp(b).reverse()
        });
        let asks = aggerate_levels(&bitstamp_book.asks, &binance_book.asks, |a, b| a.cmp(b));

        if bids.is_empty() || asks.is_empty() {
            // We can't publish a spread since the book is one-sided.
            continue;
        }

        let spread = asks[0].price - bids[0].price;

        tx.send(Summary { bids, asks, spread })?;
    }
}
