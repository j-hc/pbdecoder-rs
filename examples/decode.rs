use pbdecoder::decode_proto;

fn main() {
    let f = std::fs::read("protobuf-binary").unwrap();
    let (parts, _remaining_bytes) = decode_proto(&f);

    let s = serde_json::to_string(&parts).unwrap();
    std::fs::write("resp.json", s).unwrap();
}
