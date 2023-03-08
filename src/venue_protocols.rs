use rust_decimal::Decimal;

pub type Price = Decimal;
pub type Qty = Decimal;
pub type Level = (Price, Qty);

pub mod bitstamp {
    use serde::Deserialize;
    use serde_json::json;

    use crate::venue_protocols::*;

    #[derive(Deserialize, Debug)]
    pub struct BookUpdate {
        pub bids: Vec<Level>,
        pub asks: Vec<Level>,
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

pub mod binance {
    use serde::Deserialize;

    use crate::venue_protocols::*;

    #[derive(Deserialize, Debug)]
    pub struct BookUpdate {
        pub bids: Vec<Level>,
        pub asks: Vec<Level>,
    }
}
