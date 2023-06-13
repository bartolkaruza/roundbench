const https = require("https");
const crypto = require("crypto");
const WebSocket = require("ws");
const { PerformanceObserver, performance } = require("node:perf_hooks");

const apiKey = process.env.BINANCE_KEY;
const apiSecret = process.env.BINANCE_SECRET;

const requestOptions = {
  hostname: "fapi.binance.com",
  port: 443,
  headers: {
    "X-MBX-APIKEY": apiKey,
  },
};

function signedRequest(method, path, keys, params, onJsonParsed) {
  params.timestamp = Date.now();
  let dataQueryString = keys.map((key) => key + "=" + params[key]).join("&");
  const signature = crypto
    .createHmac("sha256", apiSecret)
    .update(dataQueryString)
    .digest("hex");
  dataQueryString += "&signature=" + encodeURIComponent(signature);
  requestOptions.path = path + dataQueryString;
  requestOptions.method = method;
  let respStr = "";
  const req = https.request(requestOptions, (res) => {
    res.on("data", (d) => {
      respStr += d;
    });
  });
  req.on("close", () => {
    onJsonParsed(JSON.parse(respStr));
  });

  req.on("error", (e) => {
    console.error(e);
  });
  req.end();
}

const placeOrderParams = {
  symbol: "BTCUSDT",
  side: "BUY",
  type: "LIMIT",
  timeInForce: "GTC",
  newClientOrderId: "roundbench",
  quantity: 0.001,
  price: 20000,
  recvWindow: 5000,
};
const placeOrderKeys = Object.keys(placeOrderParams);
placeOrderKeys.push("timestamp");

function binancePlaceOrder(onResponse) {
  signedRequest(
    "POST",
    "/fapi/v1/order?",
    placeOrderKeys,
    placeOrderParams,
    onResponse
  );
}

const cancelOrderParams = {
  symbol: "BTCUSDT",
  origClientOrderId: "roundbench",
  recvWindow: 50000,
};
const cancelOrderKeys = Object.keys(cancelOrderParams);
cancelOrderKeys.push("timestamp");

function binanceCancelOrder(onResponse) {
  signedRequest(
    "DELETE",
    "/fapi/v1/order?",
    cancelOrderKeys,
    cancelOrderParams,
    onResponse
  );
}

async function getListenKey() {
  return await new Promise((resolve) =>
    signedRequest("POST", "/fapi/v1/listenKey?", [], {}, ({ listenKey }) =>
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
      console.log("Connection opened");
      resolve();
    });

    // Listen for messages
    socket.on("message", (message) => {
      //   console.log("Message from server: ", );
      const payload = JSON.parse(message);
      if ("e" in payload && payload["e"] == "ORDER_TRADE_UPDATE") {
        if (payload["o"]["x"] == "CANCELED") {
          onOrderCancelled();
        } else if (payload["o"]["x"] == "NEW") {
          onOrderPlaced();
        }
      }
    });

    // Connection closed
    socket.on("close", () => {
      console.log("Connection closed");
    });

    // Error handling
    socket.on("error", (error) => {
      console.log("Error: ", error);
    });
  });
}

function sampleLoop() {
  const obs = new PerformanceObserver((items) => {
    console.log({ items });
    console.log(items.getEntries()[0].duration);
    // performance.clearMarks();
  });
  obs.observe({ type: "measure" });
  //   performance.measure("Start to Now");
  //   performance.mark("A");
  //   doSomeLongRunningProcess(() => {
  //     performance.measure("A to Now", "A");

  //     performance.mark("B");
  //     performance.measure("A to B", "A", "B");
  //   });

  binanceListenUserStream(
    () => {
      console.log("on order");
      performance.mark("B");
      performance.measure("A to B", "A", "B");
      binanceCancelOrder((c) => null);
    },
    () => {
      performance.mark("C");
      console.log("on cancel");
      performance.measure("B to C", "B", "C");
      performance.measure("end to end", "A", "C");
      console.log("FIN");
    }
  ).then(() => {
    performance.mark("A");
    binancePlaceOrder((o) => {
      //   performance.measure("A response", "A");
      console.log({ o });
    });
  });
}

sampleLoop();
// const sign = crypto.createSign("RSA-SHA256");
// sign.update(dataQueryString);
// sign.end();

// const signature = sign.sign(
//   { key: apiSecret, padding: crypto.constants.RSA_PKCS1_PSS_PADDING },
//   "base64"
// );

// const orderRequestOptions = {
//   hostname: "fapi.binance.com",
//   port: 443,
//   path: "/fapi/v1/order?" + dataQueryString,
//   method: "POST",
//   headers: {
//     "X-MBX-APIKEY": apiKey,
//   },
// };
