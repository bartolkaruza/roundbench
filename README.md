![roundbench](roundbench.png)

Benchmark of various programming languages for the specific usecase of high frequency trading in crypto, comparing their effectiveness for IO bound applications. This benchmark deals with one specific venue, Binance, with a single market, BTCUSDT, selected as it is the highest volume single market in crypto as of writing.

This benchmark tries to separate venue related issues, due to congestion and other sources of noise, by sampling over a longer time period. The orders that are placed are well outside of the active trading range and are all cancelled before filling, so this benchmark doesn't include a full trading lifecycle, but a sufficient portion of it for the purpose of comparing programming languages and runtimes.

The following loop with respective sampled time represents a single sample;
1. Place a limit order
2. Wait for websocket order update to NEW status
3. Cancel the order
4. Wait for websocket order update to CANCELED status

During each request and response, the full response payload is parsed from a JSON text representation into a language specific hashmap or struct data structure. You could in real world hft scenario's skip (or perform partially) this parsing to speed up processing, but for this benchmark, we assume the information is needed for decision making and parsing is not optional.

For each step start and complete time are recorded, as well as the exchange provided timing information about actual order placement and cancellation.

The above protocol is implemented in a range of different programming languages, selected for either their widespread use in high frequency trading or their potential for competitive performance.

STARTED
- Javascript, NodeJS
- Rust

TODO
- Python
- Zig
- C++
- C

This is an early work in progress version. Contributions and collaboration is more than welcome.