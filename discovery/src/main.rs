mod logger;

use {
    futures_util::{
        SinkExt,
        StreamExt,
    },
    log::{info, error, debug},
    serde_json::{
        json,
        Value,
    },
    std::collections::HashMap,
    tokio::{
        io::AsyncWriteExt,
        net::UnixStream,
        sync::mpsc,
    },
    tokio_tungstenite::{
        connect_async,
        tungstenite::protocol::Message,
    },
    logger::setup_logger,
};

const IPC_SOCKET_PATH: &'static str = "/tmp/arbitrage_ipc.sock";
const BINANCE_WS_URL: &str = "wss://stream.binance.com:443";
const BYBIT_WS_URL: &str = "wss://stream.bybit.com/v5/public/spot";
const BITMEX_WS_URL: &str = "wss://ws.bitmex.com/realtime";

#[derive(Debug, Clone)]
struct PriceData {
    exchange: &'static str,
    pair: String,
    best_bid: f64,
    best_ask: f64,
}

#[tokio::main]
async fn main() {

    setup_logger(true).expect("No logging for you.");

    let (tx, mut rx) = mpsc::channel(100);
    let mut latest_prices: HashMap<&str, PriceData> = HashMap::new();

    let symbols = vec!["SOLUSDT"];

    subscribe_to_binance(tx.clone(), symbols.clone()).await;
    subscribe_to_bybit(tx.clone(), symbols.clone(), 1).await;
    subscribe_to_bitmex(tx.clone(), symbols.clone()).await;

    while let Some(price_data) = rx.recv().await {
        latest_prices.insert(price_data.exchange, price_data);

        if latest_prices.len() >= 3 {
            detect_arbitrage_opportunity(&latest_prices).await;
        }
    }
}


async fn subscribe_to_exchange(
    exchange_name: &'static str,
    url: &str,
    subscription: Value,
    tx: mpsc::Sender<PriceData>,
    parse_message: fn(&str) -> Option<PriceData>,
) {
    let (stream, _) = connect_async(url)
        .await
        .expect(&format!("Failed to connect to WebSocket at {}", url));
    let (mut writer, mut reader) = stream.split();

    writer
        .send(Message::Text(subscription.to_string()))
        .await
        .expect(&format!("Failed to subscribe to {}", exchange_name));
    info!(
        "[{}] Sent subscription request: {}",
        exchange_name, subscription
    );

    let tx_clone = tx.clone();

    tokio::spawn(async move {
        while let Some(res) = reader.next().await {
            match res {
                Ok(Message::Text(msg)) => {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&msg) {
                        if let Some(result) = parsed.get("result") {
                            if result.is_null() {
                                info!(
                                    "[{}] Subscription successful: {}",
                                    exchange_name, subscription
                                );
                            } else {
                                info!(
                                    "[{}] Unexpected subscription response: {}",
                                    exchange_name, msg
                                );
                            }
                        } else if let Some(error) = parsed.get("error") {
                            error!(
                                "[{}] Subscription error: {}",
                                exchange_name, error
                            );
                        } else if let Some(price_data) = parse_message(&msg) {
                            info!(
                                "[{}] Received price_data: {:?}",
                                exchange_name, price_data
                            );
                            tx_clone.send(price_data).await.unwrap();
                        } else {
                            info!(
                                "[{}] Received message that did not match expected format: {}",
                                exchange_name, msg
                            );
                        }
                    }
                }
                Ok(Message::Ping(payload)) => {
                    info!("[{}] Received Ping, sending Pong", exchange_name);
                    writer.send(Message::Pong(payload)).await.unwrap();
                }
                Ok(Message::Pong(payload)) => {
                    info!("[{}] Received Pong: {:?}", exchange_name, payload);
                }
                _ => {}
            }
        }

        while let Some(Ok(Message::Text(msg))) = reader.next().await {
            if let Some(price_data) = parse_message(&msg) {
                tx_clone.send(price_data).await.unwrap();
            }
        }
    });
}


async fn subscribe_to_binance(tx: mpsc::Sender<PriceData>, symbols: Vec<&str>) {

    let streams = symbols.iter()
        .map(|s| format!("{}@bookTicker", s.to_lowercase()))
        .collect::<Vec<_>>()
        .join("/");
    let url = format!("{}/stream?streams={}", BINANCE_WS_URL, streams);

    let subscription = json!({
        "method": "SUBSCRIBE",
        "params": symbols,
        "id": 1,
    });

    subscribe_to_exchange("binance", &url, subscription, tx, parse_binance_message).await;

}

async fn subscribe_to_bybit(tx: mpsc::Sender<PriceData>, symbols: Vec<&str>, depth: usize) {

    let topics: Vec<String> = symbols.iter()
        .map(|symbol| format!("orderbook.{}.{}", depth, symbol.to_uppercase()))
        .collect();

    let subscription = json!({
        "op": "subscribe",
        "args": topics,
    });

    subscribe_to_exchange("bybit", BYBIT_WS_URL, subscription, tx, parse_bybit_message).await;

}

async fn subscribe_to_bitmex(tx: mpsc::Sender<PriceData>, symbols: Vec<&str>) {
    let topics: Vec<String> = symbols
        .iter()
        .map(|symbol| format!("quote:{}", symbol.to_uppercase()))
        .collect();

    let subscription = json!({
        "op": "subscribe",
        "args": topics,
    });

    subscribe_to_exchange("bitmex", BITMEX_WS_URL, subscription, tx, parse_bitmex_message).await;
}

fn parse_binance_message(msg: &str) -> Option<PriceData> {
    if let Ok(parsed) = serde_json::from_str::<Value>(msg) {
        if let Some(stream) = parsed.get("stream").and_then(|v| v.as_str()) {
            if stream.ends_with("@bookTicker") {
                if let Some(data) = parsed.get("data") {
                    if let (Some(symbol), Some(best_bid_str), Some(best_ask_str)) = (
                        data.get("s").and_then(|v| v.as_str()),
                        data.get("b").and_then(|v| v.as_str()),
                        data.get("a").and_then(|v| v.as_str()),
                    ) {
                        let best_bid = best_bid_str.parse::<f64>().ok()?;
                        let best_ask = best_ask_str.parse::<f64>().ok()?;

                        return Some(PriceData {
                            exchange: "Binance",
                            pair: symbol.to_string(),
                            best_bid,
                            best_ask,
                        });
                    }
                }
            }
        }
    }
    None
}

fn parse_bybit_message(msg: &str) -> Option<PriceData> {
    use serde_json::Value;

    if let Ok(parsed) = serde_json::from_str::<Value>(msg) {
        if let Some(topic) = parsed.get("topic").and_then(Value::as_str) {
            if topic.starts_with("orderbook.") {
                if let Some(data) = parsed.get("data") {
                    if let Some(symbol) = data.get("s").and_then(Value::as_str) {

                        let mut best_bid = None;
                        let mut best_ask = None;

                        // extract best bid
                        if let Some(bids) = data.get("b").and_then(Value::as_array) {
                            if !bids.is_empty() {
                                if let Some(bid_entry) = bids.get(0) {
                                    if let Some(bid_price_str) = bid_entry.get(0).and_then(Value::as_str) {
                                        if let Ok(bid_price) = bid_price_str.parse::<f64>() {
                                            best_bid = Some(bid_price);
                                        }
                                    }
                                }
                            }
                        }

                        // extract best ask
                        if let Some(asks) = data.get("a").and_then(Value::as_array) {
                            if !asks.is_empty() {
                                if let Some(ask_entry) = asks.get(0) {
                                    if let Some(ask_price_str) = ask_entry.get(0).and_then(Value::as_str) {
                                        if let Ok(ask_price) = ask_price_str.parse::<f64>() {
                                            best_ask = Some(ask_price);
                                        }
                                    }
                                }
                            }
                        }

                        // Return PriceData only if both best_bid and best_ask are available
                        if let (Some(best_bid), Some(best_ask)) = (best_bid, best_ask) {
                            return Some(PriceData {
                                exchange: "Bybit",
                                pair: symbol.to_string(),
                                best_bid,
                                best_ask,
                            });
                        }
                        // If one side is missing, we cannot produce a complete PriceData
                        // It's acceptable to skip this message and wait for the next one
                    }
                }
            }
        }
    }
    None
}



fn parse_bitmex_message(msg: &str) -> Option<PriceData> {
    if let Ok(parsed) = serde_json::from_str::<Value>(msg) {
        if parsed["table"] == "quote" {
            if let Some(data_array) = parsed["data"].as_array() {
                for data in data_array {
                    if let (Some(symbol), Some(best_bid), Some(best_ask)) = (
                        data.get("symbol").and_then(|v| v.as_str()),
                        data.get("bidPrice").and_then(|v| v.as_f64()),
                        data.get("askPrice").and_then(|v| v.as_f64()),
                    ) {
                        return Some(PriceData {
                            exchange: "BitMEX",
                            pair: symbol.to_string(),
                            best_bid,
                            best_ask,
                        });
                    }
                }
            }
        }
    }
    None
}


async fn detect_arbitrage_opportunity(exchanges: &HashMap<&str, PriceData>) {
    let exchanges_list = vec!["Binance", "Bybit", "BitMEX"];

    let mut arbitrage_opportunities = Vec::new();

    for &buy_exchange_name in &exchanges_list {
        for &sell_exchange_name in &exchanges_list {
            if buy_exchange_name == sell_exchange_name {
                continue;
            }

            if let (Some(buy_exchange), Some(sell_exchange)) = (
                exchanges.get(buy_exchange_name),
                exchanges.get(sell_exchange_name),
            ) {
                debug!(
                    "Comparing Buy on {} at {:.2} (ask) with Sell on {} at {:.2} (bid)",
                    buy_exchange.exchange, buy_exchange.best_ask,
                    sell_exchange.exchange, sell_exchange.best_bid
                );

                if buy_exchange.best_ask < sell_exchange.best_bid {
                    info!(
                        "Arbitrage opportunity: Buy on {} at {:.2}, Sell on {} at {:.2}",
                        buy_exchange.exchange, buy_exchange.best_ask,
                        sell_exchange.exchange, sell_exchange.best_bid
                    );
                    arbitrage_opportunities.push(vec![buy_exchange.clone(), sell_exchange.clone()]);
                } else {
                    debug!(
                        "No arbitrage: Buy on {} at {:.2}, Sell on {} at {:.2}",
                        buy_exchange.exchange, buy_exchange.best_ask,
                        sell_exchange.exchange, sell_exchange.best_bid
                    );
                }
            }
        }
    }

    for opportunity in arbitrage_opportunities {
        send_ipc_message(&opportunity).await;
    }
}

async fn send_ipc_message(arb_opportunity: &Vec<PriceData>) {
    match UnixStream::connect(IPC_SOCKET_PATH).await {
        Ok(mut socket) => {
            let trade_info: Vec<Value> = arb_opportunity
                .iter()
                .map(|data| {
                    json!({
                        "exchange": data.exchange,
                        "pair": data.pair,
                        "best_bid": data.best_bid,
                        "best_ask": data.best_ask,
                    })
                })
                .collect();

            let msg = json!({
                "arbitrage_opportunity": trade_info,
            });
            let msg_str = msg.to_string();

            if let Err(e) = socket.write_all(msg_str.as_bytes()).await {
                error!("Failed to send message via IPC: {}", e);
            } else {
                info!("Sent arbitrage opportunity via IPC: {}", msg_str);
            }
        }
        Err(e) => {
            error!("Failed to connect to IPC socket: {}", e);
        }
    }
}
