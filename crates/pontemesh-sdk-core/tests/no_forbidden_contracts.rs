use std::fs;
use std::path::Path;

#[test]
fn sdk_core_does_not_use_s3_mcp_or_admin_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut haystack = String::new();
    collect_rs(&root, &mut haystack);

    for forbidden in [
        "SigV4",
        "ListBuckets",
        "PutObject",
        "DeleteObject",
        "/mcp",
        "MCP",
        "/api/admin",
    ] {
        assert!(
            !haystack.contains(forbidden),
            "SDK core must not contain forbidden contract {forbidden}"
        );
    }
}

fn collect_rs(path: &Path, output: &mut String) {
    for entry in fs::read_dir(path).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, output);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            output.push_str(&fs::read_to_string(path).expect("read source"));
        }
    }
}
