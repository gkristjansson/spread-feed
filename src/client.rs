use tonic::Request;

use crate::service::{orderbook_aggregator_client::OrderbookAggregatorClient, Empty};

pub mod service {
    tonic::include_proto!("orderbook");
}

#[tokio::main]
async fn main() {
    let mut client = OrderbookAggregatorClient::connect("http://[::1]:10000")
        .await
        .unwrap();

    let response = client
        .book_summary(Request::new(Empty::default()))
        .await
        .unwrap();
    let mut inbound = response.into_inner();

    while let Some(book) = inbound.message().await.unwrap() {
        print!("{}[2J", 27 as char);
        println!("Spread: {} ", book.spread);
        println!("-------------------------------------------------------------------");

        for (bid, ask) in book.bids.iter().zip(book.asks) {
            println!(
                "{:<12 } {:<12} - {:.10}        |        {:>10}  - {:<12} {:<12}",
                bid.exchange, bid.amount, bid.price, ask.price, ask.amount, ask.exchange
            );
        }
    }
}
