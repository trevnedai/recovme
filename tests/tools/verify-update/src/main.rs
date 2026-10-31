//! Checks a RecovMe release's update signatures with the same library the app's
//! updater uses (minisign-verify), against the public key in tauri.conf.json.
//! Usage: verify-update <tauri.conf.json> <latest.json> <mac .app.tar.gz> <windows -setup.exe>
use base64::Engine;

fn decode(s: &str) -> String {
    String::from_utf8(base64::engine::general_purpose::STANDARD.decode(s.trim()).unwrap()).unwrap()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let conf: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let pk = minisign_verify::PublicKey::decode(&decode(conf["plugins"]["updater"]["pubkey"].as_str().unwrap())).unwrap();
    let latest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
    println!("latest.json version: {}", latest["version"]);
    for (platform, file) in [("darwin-aarch64", &a[3]), ("windows-x86_64", &a[4])] {
        let sig = minisign_verify::Signature::decode(&decode(latest["platforms"][platform]["signature"].as_str().unwrap())).unwrap();
        let data = std::fs::read(file).unwrap();
        let ok = pk.verify(&data, &sig, false).is_ok();
        let mut tampered = data.clone();
        tampered.push(b'x');
        let rejected = pk.verify(&tampered, &sig, false).is_err();
        println!("{platform}: signature {} | changed copy {}", if ok { "OK" } else { "BAD" }, if rejected { "rejected (good)" } else { "ACCEPTED (bad)" });
    }
}
