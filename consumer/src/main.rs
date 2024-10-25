mod logger;

use {
    tokio::{
        net::UnixListener,
        io::AsyncReadExt,
        signal,
    },
    log::{info, error},
    std::path::Path,
    logger::setup_logger,
    serde_json::Value,
};

const IPC_SOCKET_PATH: &'static str = "/tmp/arbitrage_ipc.sock";

async fn start_ipc_listener() -> std::io::Result<()> {
    let socket_path = "/tmp/arbitrage_ipc.sock";

    // Remove existing socket file to avoid binding errors
    if Path::new(socket_path).exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    info!("IPC listener started at {}", socket_path);

    loop {
        match listener.accept().await {
            Ok((mut socket, _)) => {
                tokio::spawn(async move {
                    let mut buffer = vec![0; 4096]; // Increased buffer size
                    match socket.read(&mut buffer).await {
                        Ok(n) if n > 0 => {
                            let message = String::from_utf8_lossy(&buffer[..n]);
                            // Call the parsing function
                            if let Err(e) = parse_arbitrage_message(&message) {
                                error!("Failed to parse arbitrage message: {}", e);
                            }
                        }
                        Ok(_) => {
                            // Connection closed
                        }
                        Err(e) => {
                            error!("Failed to read from socket: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept IPC connection: {}", e);
            }
        }
    }
}

fn parse_arbitrage_message(message: &str) -> Result<(), Box<dyn std::error::Error>> {

    let parsed_msg: Value = serde_json::from_str(message)?;

    if let Some(arbitrage_opportunities) = parsed_msg
        .get("arbitrage_opportunity")
        .and_then(|v| v.as_array())
    {
        if arbitrage_opportunities.len() >= 2 {

            let buy_opportunity = &arbitrage_opportunities[0];
            let sell_opportunity = &arbitrage_opportunities[1];

            let buy_exchange = buy_opportunity
                .get("exchange")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let buy_price = buy_opportunity
                .get("best_ask")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let sell_exchange = sell_opportunity
                .get("exchange")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let sell_price = sell_opportunity
                .get("best_bid")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let pair = buy_opportunity
                .get("pair")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            if buy_price > 0.0 && sell_price > 0.0 {
                let price_difference = sell_price - buy_price;
                let percentage_difference = (price_difference / buy_price) * 100.0;

                info!(
                    "Arbitrage Opportunity Detected for {}:\n\
                    Buy on {} at {:.2}, Sell on {} at {:.2}\n\
                    Profit per unit: {:.2}\n\
                    Percentage Profit: {:.4}%",
                    pair,
                    buy_exchange,
                    buy_price,
                    sell_exchange,
                    sell_price,
                    price_difference,
                    percentage_difference
                );
            } else {
                error!("Invalid price data in arbitrage opportunity");
            }
        } else {
            error!("Not enough data in arbitrage opportunity");
        }
    } else {
        error!("Invalid arbitrage opportunity format");
    }

    Ok(())
}


#[tokio::main]
async fn main() {

    setup_logger(true).expect("No logging for you.");

    let _ = tokio::spawn(async {
        if let Err(e) = start_ipc_listener().await {
            error!("Failed to start IPC listener: {}", e);
        }
    });

    signal::ctrl_c().await.expect("Failed to listen for ctrl_c");

    let socket_path = IPC_SOCKET_PATH;

    if std::path::Path::new(socket_path).exists() {
        std::fs::remove_file(socket_path).expect("Failed to remove socket file");
    }

    info!("Application shutting down");

}
