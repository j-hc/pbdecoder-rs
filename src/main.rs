use protodeck::decode_proto;

fn main() {
    let f = std::fs::read("proto-example").unwrap();
    let parts = decode_proto(&f).0;

    let s = serde_json::to_string(&parts).unwrap();
    std::fs::write("resp.json", s).unwrap();
}
