use fileblade::common::parse_path;
use fileblade::core_modules::canonical::{
    canonical_json, compact_ascii_json, fingerprint, path_id, python_float_hex, python_float_repr,
    scoped_path_id, sha256_hex, short_digest, stable_id, stable_id_bytes,
};
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn baseline(name: &str) -> Value {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/python-baseline");
    let text = std::fs::read_to_string(root.join(name)).expect("baseline fixture");
    let sandbox = serde_json::from_str::<Value>(
        &std::fs::read_to_string(root.join("manifest.json")).expect("baseline manifest"),
    )
    .expect("baseline manifest json")["root"]
        .as_str()
        .expect("baseline root")
        .to_string();
    serde_json::from_str(&text.replace("{ROOT}", &sandbox)).expect("baseline json")
}

#[test]
fn ascii_encoding_matches_python_json_dumps() {
    assert_eq!(canonical_json(&json!("é")), "\"\\u00e9\"");
    assert_eq!(canonical_json(&json!("\u{1f600}")), "\"\\ud83d\\ude00\"");
    assert_eq!(
        canonical_json(&json!("line\n\ttab\u{7f}\"\\")),
        "\"line\\n\\ttab\\u007f\\\"\\\\\""
    );
    assert_eq!(
        canonical_json(&json!({"b": 1, "a": [true, false, null]})),
        "{\"a\":[true,false,null],\"b\":1}"
    );
    assert_eq!(
        compact_ascii_json(&json!({"b": 1, "a": 2})),
        "{\"b\":1,\"a\":2}",
        "the compact encoder keeps insertion order, as json.dumps does"
    );
}

#[test]
fn float_text_matches_python_repr_and_hex() {
    for (value, expected) in [
        (1.0f64, "1.0"),
        (-0.0, "-0.0"),
        (0.5, "0.5"),
        (1e16, "1e+16"),
        (1e15, "1000000000000000.0"),
        (1e-5, "1e-05"),
        (0.0001, "0.0001"),
        (1.5e300, "1.5e+300"),
        (-2.25, "-2.25"),
    ] {
        assert_eq!(python_float_repr(value), expected, "{value}");
    }
    assert_eq!(python_float_hex(1.0), "0x1.0000000000000p+0");
    assert_eq!(python_float_hex(-2.5), "-0x1.4000000000000p+1");
    assert_eq!(python_float_hex(0.0), "0x0.0p+0");
}

#[test]
fn hashing_helpers_match_the_python_shapes() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(stable_id(&["scope", "/path"]).len(), 16);
    assert_eq!(
        stable_id(&["scope", "/path"]),
        sha256_hex(b"scope\0/path")[..16].to_string()
    );
    assert_eq!(short_digest("value"), sha256_hex(b"value")[..8].to_string());
    assert_eq!(
        fingerprint(&json!({"a": 1})),
        sha256_hex(b"{\"a\":1}").to_string()
    );
}

#[test]
fn golden_memory_row_ids_reproduce_from_their_realpath() {
    let document = baseline("memory/list.json");
    let items = document["items"].as_array().expect("memory items");
    assert!(!items.is_empty());
    for row in items {
        let realpath = row["realpath"].as_str().expect("memory realpath");
        assert_eq!(
            path_id(&parse_path(realpath).expect("memory realpath parses")),
            row["id"].as_str().expect("memory id"),
            "{realpath}"
        );
    }
}

#[test]
fn golden_skills_row_ids_reproduce_from_scope_and_path() {
    let document = baseline("skills/list.json");
    let items = document["items"].as_array().expect("skills items");
    assert!(!items.is_empty());
    let mut native = 0;
    for row in items {
        let scope = row["scope"].as_str().expect("skills scope");
        let link = row["link_target"].as_str().expect("skills link target");
        let raw = if link.is_empty() {
            row["path"].as_str().expect("skills path")
        } else {
            link
        };
        let path = parse_path(raw).expect("skills path parses");
        if path.to_str().is_none() {
            native += 1;
        }
        assert_eq!(
            scoped_path_id(scope, &path),
            row["id"].as_str().expect("skills id"),
            "{raw}"
        );
    }
    assert!(native > 0, "the baseline must carry a native-byte path");
}

#[test]
fn row_ids_hash_the_raw_path_bytes_rather_than_a_lossy_rendering() {
    let path = Path::new(OsStr::from_bytes(b"/tmp/repo-\xff/SKILL.md"));
    let mut preimage = b"project\0/tmp/repo-".to_vec();
    preimage.push(0xff);
    preimage.extend_from_slice(b"/SKILL.md");
    assert_eq!(
        scoped_path_id("project", path),
        sha256_hex(&preimage)[..16].to_string()
    );
    assert_ne!(
        scoped_path_id("project", path),
        stable_id(&["project", &path.to_string_lossy()])
    );
    assert_eq!(
        path_id(path),
        sha256_hex(path.as_os_str().as_bytes())[..16].to_string()
    );
    assert_eq!(stable_id_bytes(&[b"a", b"b"]), stable_id(&["a", "b"]));
}

#[test]
fn golden_mcp_definition_fingerprint_reproduces_from_its_raw_configuration() {
    let record = baseline("mcp/recovery-record.json");
    let payload = &record["payload"];
    assert_eq!(
        fileblade::core_modules::mcp::value::fingerprint_json(&payload["raw"]),
        payload["definition"].as_str().expect("definition digest")
    );
}
