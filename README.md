# pbdecoder

Decode raw protobuf binaries into JSON (or into any other schema) without having the proto files

```toml
# Cargo.toml

pbdecoder = { git = "https://github.com/j-hc/pbdecoder-rs.git" }
```

```rust
use pbdecoder::decode_proto;

let f = std::fs::read("protobuf-binary").unwrap();
let (parts, remaining_bytes) = decode_proto(&f);

let s = serde_json::to_string(&parts).unwrap();
std::fs::write("resp.json", s).unwrap();
```