# pbdecoder

```rust
use pbdecoder::decode_proto;

let f = std::fs::read("raw-proto-binary").unwrap();
let parts = decode_proto(&f).0;

let s = serde_json::to_string(&parts).unwrap();
std::fs::write("resp.json", s).unwrap();
```