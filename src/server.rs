mod aggregator;
mod iter_utils;
mod venue_protocols;

use std::pin::Pin;

use clap::Parser;
use futures_core::Stream;
use tokio::{
    select,
    sync::{broadcast, broadcast::Sender},
};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tonic::{transport::Server, Response, Status};

use crate::service::{
    orderbook_aggregator_server::{OrderbookAggregator, OrderbookAggregatorServer},
    Empty, Summary,
};

pub mod service {
    tonic::include_proto!("orderbook");
}

#[derive(Debug)]
struct Service {
    tx: Sender<Summary>,
}

#[tonic::async_trait]
impl OrderbookAggregator for Service {
    type BookSummaryStream =
        Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Send + Sync + 'static>>;

    async fn book_summary(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        let rx = self.tx.subscribe();
        let mut stream = BroadcastStream::new(rx);

        let output = async_stream::try_stream! {
            while let Some(Ok(summary)) = stream.next().await {
                yield summary
            }
        };

        Ok(Response::new(Box::pin(output) as Self::BookSummaryStream))
    }
}

#[derive(Parser, Debug)]
struct Args {
    #[clap(long, default_value = "ethbtc")]
    symbol: String,
}

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();

    let (tx, _rx) = broadcast::channel(16);

    let addr = "[::1]:10000".parse().unwrap();

    let service = Service { tx: tx.clone() };

    let svc = OrderbookAggregatorServer::new(service);

    select! {
        e = Server::builder().add_service(svc).serve(addr) => {
            println!("Server exited, {:?}", e);
        }
       e = aggregator::aggregator_task(args.symbol, tx) => {
            println!("Aggregator exited, {:?}", e);
        }
    }
}
