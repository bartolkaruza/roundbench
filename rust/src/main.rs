use hex;
use hyper::{Body, Client, Method, Request};
use hyper_tls::HttpsConnector;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

use ring::hmac;
use std::env;
use std::process;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use futures_util::StreamExt;
use tokio_tungstenite::connect_async;

// Instant.

#[derive(Serialize, Deserialize)]
struct Sample {
    start: Option<[i64; 2]>,
    place_response: Option<[i64; 2]>,
    place_json_parsed: Option<[i64; 2]>,
    ws_place: Option<[i64; 2]>,
    ws_cancel_parsed: Option<[i64; 2]>,
    cancel_response: Option<[i64; 2]>,
    cancel_json_parsed: Option<[i64; 2]>,
    ws_place_parsed: Option<[i64; 2]>,
    end: Option<[i64; 2]>,
}

async fn secure_request(
    api_key: &str,
    api_secret: &str,
    data_query_string: &str,
    resource: &str,
    method: Method,
) -> Result<(hyper::StatusCode, Value), Box<dyn std::error::Error + Send + Sync>> {
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
) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
    let (status, v) = secure_request(api_key, api_secret, "", "listenKey", Method::POST).await?;
    let listen_key = v["listenKey"]
        .as_str()
        .unwrap()
        .trim_matches('"')
        .to_string();
    // println!("{}", listen_key);
    let url = format!("wss://fstream.binance.com/ws/{}", listen_key);
    // println!("{}", url);
    let (mut ws_stream, _) = connect_async(&url).await.expect("Failed to connect");
    // println!("Connected to WebSocket!");

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
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    // println!("Error: {}", e);
                    break;
                }
            }
        }
    });
    Ok(read_handle)
}

fn hr_time() -> [i64; 2] {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let secs = duration.as_secs() as i64;
    let nano_secs = duration.subsec_nanos() as i64;
    return [secs, nano_secs];
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut sample = Sample {
        start: None,
        place_response: None,
        place_json_parsed: None,
        ws_place: None,
        ws_cancel_parsed: None,
        cancel_response: None,
        cancel_json_parsed: None,
        ws_place_parsed: None,
        end: None,
    };
    let data = Arc::new(Mutex::new(sample));

    let api_key: String = env::var("BINANCE_KEY").expect("KEY NOT SET");
    let api_secret: String = env::var("BINANCE_SECRET").expect("SECRET NOT SET");

    // let timestamp = SystemTime::now();
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let timestamp = since_the_epoch.as_millis();

    // Create the channels.
    let (new_tx, mut new_rx) = mpsc::channel::<BusMessage>(100);
    let (canceled_tx, mut canceled_rx) = mpsc::channel::<BusMessage>(100);

    let api_key_clone = api_key.clone();
    let api_key_secret = api_secret.clone();

    let data1 = Arc::clone(&data);
    let data2 = Arc::clone(&data);
    let new_received_handle = tokio::spawn(async move {
        while let Some(msg) = new_rx.recv().await {
            {
                let mut data: tokio::sync::MutexGuard<'_, Sample> = data1.lock().await;
                data.ws_place_parsed = Some(hr_time());
            }
            let cancel_query_string = format!(
                "symbol=BTCUSDT&origClientOrderId=roundbench_rust&recvWindow=50000&timestamp={timestamp}"
            );
            let res = secure_request(
                &api_key_clone,
                &api_key_secret,
                &cancel_query_string,
                "order",
                Method::DELETE,
            )
            .await;

            let mut data = data2.lock().await;
            match res {
                Ok((status, body)) => {
                    data.cancel_json_parsed = Some(hr_time());
                    // println!("{} - {}", status, body);
                    // println!("{}", start_time.elapsed().as_millis());
                }
                Err(e) => {
                    eprintln!("Error when sending request: {:?}", e);
                }
            }
            break;
        }
    });

    let data3 = Arc::clone(&data);
    let cancel_received_handle = tokio::spawn(async move {
        while let Some(msg) = canceled_rx.recv().await {
            let mut data = data3.lock().await;
            // println!("cancel received! {}", data);
            data.ws_cancel_parsed = Some(hr_time());

            println!("{}", serde_json::to_string(&*data).unwrap());
            break;
        }
    });

    let handle: JoinHandle<()> =
        connect_user_stream(&api_key, &api_secret, new_tx, canceled_tx).await?;

    {
        let mut data_guard = data.lock().await;
        data_guard.start = Some(hr_time());
    }

    let order_query_string = format!("symbol=BTCUSDT&side=BUY&type=LIMIT&timeInForce=GTC&newClientOrderId=roundbench_rust&quantity=0.001&price=20000&recvWindow=5000&timestamp={timestamp}");
    let place_resp = secure_request(
        &api_key,
        &api_secret,
        &order_query_string,
        "order",
        Method::POST,
    )
    .await;
    {
        let mut data_guard = data.lock().await;
        data_guard.place_json_parsed = Some(hr_time());
    }

    // println!("{} - {}", place_resp.0, place_resp.1);
    // println!("{}", start_time.elapsed().as_millis());

    let _ = handle.await;
    let _ = cancel_received_handle.await;
    let _ = new_received_handle.await;
    {
        let mut data_guard = data.lock().await;
        data_guard.end = Some(hr_time());
    }
    process::exit(0);

    Ok(())
}
