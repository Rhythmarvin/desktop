//! Integration tests that validate the Rust frame codec and Agent Contract
//! types against the canonical golden fixtures in `fixtures/v1/`.

use ora_plugin_protocol::frame::{decode_header, encode_frame, FrameType, HEADER_LEN};
use ora_plugin_protocol::MAX_FRAME_BYTES;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct FrameGolden {
    #[serde(rename = "fixtureVersion")]
    fixture_version: u32,
    #[serde(rename = "wireVersion")]
    wire_version: u32,
    #[serde(rename = "maximumPayloadBytes")]
    maximum_payload_bytes: u32,
    valid: Vec<FrameEntry>,
    invalid: Vec<InvalidFrameEntry>,
}

#[derive(Debug, Deserialize)]
struct FrameEntry {
    #[serde(rename = "frameType")]
    frame_type: String,
    #[serde(rename = "payloadUtf8")]
    payload_utf8: String,
    #[serde(rename = "payloadLen")]
    payload_len: u32,
    #[serde(rename = "headerHex")]
    header_hex: String,
    #[serde(rename = "frameHex")]
    frame_hex: String,
}

#[derive(Debug, Deserialize)]
struct InvalidFrameEntry {
    #[serde(rename = "frameHex")]
    frame_hex: String,
    reason: String,
}

fn frame_type_from_str(s: &str) -> FrameType {
    match s {
        "request" => FrameType::Request,
        "response" => FrameType::Response,
        "notification" => FrameType::Notification,
        _ => panic!("unknown frame type: {s}"),
    }
}

#[test]
fn frame_golden_valid_entries_round_trip() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v1/frame-golden.json"
    );
    let json = fs::read_to_string(path).expect("failed to read frame-golden.json");
    let golden: FrameGolden = serde_json::from_str(&json).expect("failed to parse frame-golden.json");

    assert_eq!(golden.fixture_version, 1);
    assert_eq!(golden.wire_version, 1);
    assert_eq!(golden.maximum_payload_bytes, 8_388_608);

    for entry in &golden.valid {
        let frame_type = frame_type_from_str(&entry.frame_type);

        // Verify payload length
        assert_eq!(
            entry.payload_utf8.len() as u32,
            entry.payload_len,
            "payload length mismatch for {} frame",
            entry.frame_type
        );

        // Verify header hex
        let frame = encode_frame(frame_type, entry.payload_utf8.as_bytes(), MAX_FRAME_BYTES)
            .unwrap_or_else(|e| panic!("failed to encode {} frame: {e}", entry.frame_type));
        let header_bytes = &frame[..HEADER_LEN];
        let actual_header_hex = hex::encode(header_bytes);
        assert_eq!(
            actual_header_hex, entry.header_hex,
            "header hex mismatch for {} frame",
            entry.frame_type
        );

        // Verify decode_header round-trip
        let header: [u8; HEADER_LEN] = header_bytes.try_into().unwrap();
        let (decoded_len, decoded_type) =
            decode_header(&header).expect("failed to decode header");
        assert_eq!(decoded_len, entry.payload_len as i32);
        assert_eq!(decoded_type, frame_type);

        // Verify full frame hex
        let actual_frame_hex = hex::encode(&frame);
        assert_eq!(
            actual_frame_hex, entry.frame_hex,
            "frame hex mismatch for {} frame",
            entry.frame_type
        );
    }
}

#[test]
fn frame_golden_invalid_headers_rejected() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v1/frame-golden.json"
    );
    let json = fs::read_to_string(path).expect("failed to read frame-golden.json");
    let golden: FrameGolden = serde_json::from_str(&json).expect("failed to parse frame-golden.json");

    for entry in &golden.invalid {
        let frame_bytes =
            hex::decode(&entry.frame_hex).expect("failed to decode frame hex");
        // Must have at least a full header
        if frame_bytes.len() >= HEADER_LEN {
            let header: [u8; HEADER_LEN] = frame_bytes[..HEADER_LEN].try_into().unwrap();
            let result = decode_header(&header);
            assert!(
                result.is_err(),
                "invalid frame '{}' ({}) should be rejected",
                entry.frame_hex,
                entry.reason
            );
        }
    }
}

#[test]
fn agent_contract_golden_parses() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v1/agent-contract-golden.json"
    );
    let json = fs::read_to_string(path).expect("failed to read agent-contract-golden.json");
    let golden: serde_json::Value =
        serde_json::from_str(&json).expect("failed to parse agent-contract-golden.json");

    let obj = golden.as_object().expect("golden should be a JSON object");
    assert_eq!(obj["version"], 1, "version should be 1");
    let entries = &obj["entries"];
    assert!(entries["requests"].is_object(), "requests should be present");
    assert!(entries["results"].is_object(), "results should be present");
    assert!(entries["events"].is_object(), "events should be present");
    assert!(entries["errors"].is_object(), "errors should be present");
}
