const https = require("https");
const crypto = require("crypto");
const WebSocket = require("ws");

const apiKey = process.env.BINANCE_KEY;
const apiSecret = process.env.BINANCE_SECRET;

const sample = {};

const requestOptions = {
  hostname: "fapi.binance.com",
  port: 443,
  headers: {
    "X-MBX-APIKEY": apiKey,
  },
};

function signedRequest(
  method,
  path,
  queryString,
  onJsonParsed,
  onResponse = null,
  onHeader = null
) {
  const signature = crypto
    .createHmac("sha256", apiSecret)
    .update(queryString)
    .digest("hex");
  requestOptions.path = `${path}${queryString}&signature=${encodeURIComponent(
    signature
  )}`;
  requestOptions.method = method;
  let respStr = "";
  const req = https.request(requestOptions, (res) => {
    res.on("data", (d) => {
      respStr += d;
    });
    res.on("end", () => {
      if (onResponse !== null) {
        onResponse();
      }
      onJsonParsed(JSON.parse(respStr));
    });
  });
  if (onHeader !== null) {
    req.on("connect", () => {});
  }

  req.on("error", (e) => {
    // console.error(e);
  });
  req.end();
}

function binancePlaceOrder(onHeader, onResponse, onJsonParsed) {
  signedRequest(
    "POST",
    "/fapi/v1/order?",
    `symbol=BTCUSDT&side=BUY&type=LIMIT&timeInForce=GTC&newClientOrderId=roundbench_rust&quantity=0.001&price=20000&recvWindow=5000&timestamp=${Date.now()}`,
    onJsonParsed,
    onResponse,
    onHeader
  );
}

function binanceCancelOrder(onHeader, onResponse, onJsonParsed) {
  signedRequest(
    "DELETE",
    "/fapi/v1/order?",
    `symbol=BTCUSDT&origClientOrderId=roundbench_rust&recvWindow=50000&timestamp=${Date.now()}`,
    onJsonParsed,
    onResponse,
    onHeader
  );
}

async function getListenKey() {
  return await new Promise((resolve) =>
    signedRequest("POST", "/fapi/v1/listenKey?", "", ({ listenKey }) =>
      resolve(listenKey)
    )
  );
}

async function binanceListenUserStream(onOrderPlaced, onOrderCancelled) {
  const listenKey = await getListenKey();
  // Create WebSocket connection.
  return await new Promise((resolve) => {
    const socket = new WebSocket(`wss://fstream.binance.com/ws/${listenKey}`);

    // Connection opened
    socket.on("open", () => {
      resolve();
    });

    // Listen for messages
    socket.on("message", (message) => {
      if (!("ws_order_update" in sample)) {
        sample["ws_place"] = process.hrtime();
      } else {
        sample["ws_cancel"] = process.hrtime();
      }
      const payload = JSON.parse(message);
      if ("e" in payload && payload["e"] == "ORDER_TRADE_UPDATE") {
        if (payload["o"]["x"] == "CANCELED") {
          sample["ws_place_parsed"] = process.hrtime();
          onOrderCancelled();
        } else if (payload["o"]["x"] == "NEW") {
          sample["ws_cancel_parsed"] = process.hrtime();
          onOrderPlaced();
        }
      }
    });

    // Connection closed
    socket.on("close", () => {
      //   console.log("Connection closed");
    });

    // Error handling
    socket.on("error", (error) => {
      //   console.log("Error: ", error);
    });
  });
}

function sampleLoop() {
  sample["start"] = process.hrtime();

  binanceListenUserStream(
    () => {
      binanceCancelOrder(
        (c) => {
          sample["cancel_response_header"] = process.hrtime();
        },
        (c) => {
          sample["cancel_response"] = process.hrtime();
        },
        (c) => {
          sample["cancel_json_parsed"] = process.hrtime();
        }
      );
    },
    () => {
      sample["end"] = process.hrtime();
      console.log(JSON.stringify(sample));

      process.exit(0);
    }
  ).then(() => {
    binancePlaceOrder(
      (c) => {
        sample["place_response_header"] = process.hrtime();
      },
      (o) => {
        sample["place_response"] = process.hrtime();
      },
      (o) => {
        sample["place_json_parsed"] = process.hrtime();
      }
    );
  });
}

sampleLoop();
