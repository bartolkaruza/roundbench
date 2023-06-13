use hex;
use hyper::{Body, Client, Method, Request};
use hyper_tls::HttpsConnector;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

use ring::hmac;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task::JoinHandle;

use std::time::Instant;

use futures_util::StreamExt;
use tokio_tungstenite::connect_async;

async fn secure_request(
    api_key: &str,
    api_secret: &str,
    data_query_string: &str,
    resource: &str,
    method: Method,
) -> Result<(hyper::StatusCode, Value), Box<dyn std::error::Error>> {
    let key_value = api_secret.as_bytes();
    let key = hmac::Key::new(hmac::HMAC_SHA256, key_value);
    let message = data_query_string.as_bytes();
    let signature = hmac::sign(&key, message);
    let hex_signature = hex::encode(signature.as_ref());

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    let uri = format!(
        "https://fapi.binance.com/fapi/v1/{resource}?{data_query_string}&signature={hex_signature}"
    );

    let req = Request::builder()
        .method(method)
        .header("X-MBX-APIKEY", api_key)
        .uri(uri)
        .body(Body::from(""))
        .expect("request builder");

    let res = client.request(req).await?;
    let status_code = res.status();
    let body_bytes = hyper::body::to_bytes(res.into_body()).await?;
    let v: Value = serde_json::from_slice(&body_bytes)?;
    // let body_string = String::from_utf8(body_bytes.to_vec())?;

    Ok((status_code, v))
}

#[derive(Debug)]
struct BusMessage {
    event: String,
    content: String,
}

async fn connect_user_stream(
    api_key: &str,
    api_secret: &str,
    new_tx: Sender<BusMessage>,
    canceled_tx: Sender<BusMessage>,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    let (status, v) = secure_request(api_key, api_secret, "", "listenKey", Method::POST).await?;
    let listen_key = v["listenKey"]
        .as_str()
        .unwrap()
        .trim_matches('"')
        .to_string();
    println!("{}", listen_key);
    let url = format!("wss://fstream.binance.com/ws/{}", listen_key);
    println!("{}", url);
    let (mut ws_stream, _) = connect_async(&url).await.expect("Failed to connect");
    println!("Connected to WebSocket!");

    let read_handle = tokio::spawn(async move {
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(msg) => {
                    if msg.is_text() || msg.is_binary() {
                        let v: serde_json::Value =
                            serde_json::from_str(&msg.to_string()).expect("json parse failure");
                        let event = v["e"].as_str().unwrap_or("");
                        let content = msg.to_string();
                        let message = BusMessage {
                            event: event.to_string(),
                            content,
                        };
                        match event {
                            "ORDER_TRADE_UPDATE" => {
                                let status = v["o"]["x"].as_str().unwrap_or("");
                                match status {
                                    "NEW" => {
                                        new_tx.send(message).await.unwrap();
                                    }
                                    "CANCELED" => {
                                        canceled_tx.send(message).await.unwrap();
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    println!("Error: {}", e);
                    break;
                }
            }
        }
    });
    Ok(read_handle)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Secret key for HMAC
    let api_key: String = env::var("BINANCE_KEY").expect("KEY NOT SET");
    let api_secret: String = env::var("BINANCE_SECRET").expect("SECRET NOT SET");

    // let timestamp = SystemTime::now();
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let timestamp = since_the_epoch.as_millis();

    let start_time = Instant::now();

    // Create the channels.
    let (new_tx, mut new_rx) = mpsc::channel::<BusMessage>(100);
    let (canceled_tx, mut canceled_rx) = mpsc::channel::<BusMessage>(100);

    let api_key_clone = api_key.clone();
    let api_key_secret = api_secret.clone();

    tokio::spawn(async move {
        while let Some(msg) = new_rx.recv().await {
            println!("Received NEW: {:?}", msg);
            let cancel_query_string = format!(
                "symbol=BTCUSDT&origClientOrderId=roundbench_rust&recvWindow=50000&timestamp={timestamp}"
            );
            match secure_request(
                &api_key_clone,
                &api_key_secret,
                &cancel_query_string,
                "order",
                Method::DELETE,
            )
            .await
            {
                Ok((status, body)) => {
                    println!("{} - {}", status, body);
                    println!("{}", start_time.elapsed().as_millis());
                }
                Err(e) => {
                    eprintln!("Error when sending request: {:?}", e);
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(msg) = canceled_rx.recv().await {
            println!("{:?} {}", Instant::now(), msg.event);
        }
    });

    let handle: JoinHandle<()> =
        connect_user_stream(&api_key, &api_secret, new_tx, canceled_tx).await?;

    let order_query_string = format!("symbol=BTCUSDT&side=BUY&type=LIMIT&timeInForce=GTC&newClientOrderId=roundbench_rust&quantity=0.001&price=20000&recvWindow=5000&timestamp={timestamp}");
    let place_resp = secure_request(
        &api_key,
        &api_secret,
        &order_query_string,
        "order",
        Method::POST,
    )
    .await?;

    println!("{} - {}", place_resp.0, place_resp.1);
    println!("{}", start_time.elapsed().as_millis());

    let _ = handle.await;

    Ok(())
}
