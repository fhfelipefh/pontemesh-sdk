use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::integrity::sha256_hex;
use pontemesh_sdk_core::{
    p2p::P2pConfig, PontemeshClient, PontemeshClientConfig, SyncObjectRequest,
};

#[derive(Debug, Clone)]
struct LoggedRequest {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: String,
}

#[derive(Default)]
struct TestState {
    requests: Mutex<Vec<LoggedRequest>>,
    replica_should_fail: bool,
}

struct TestServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    state: Arc<TestState>,
}

impl TestServer {
    fn start(replica_should_fail: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        listener
            .set_nonblocking(true)
            .expect("nonblocking test server");
        let addr = listener.local_addr().expect("local addr");
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(TestState {
            requests: Mutex::new(Vec::new()),
            replica_should_fail,
        });
        let thread_stop = Arc::clone(&stop);
        let thread_state = Arc::clone(&state);
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => handle_connection(stream, addr, &thread_state),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            stop,
            thread: Some(thread),
            state,
        }
    }

    fn origin_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn requests(&self) -> Vec<LoggedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("join test server");
        }
    }
}

#[test]
fn sdk_syncs_from_replica_records_events_and_keeps_package_token_out_of_urls() {
    let server = TestServer::start(false);
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let destination = temp_dir.path().join("maps/desert-v3.pak");
    let client = PontemeshClient::new(PontemeshClientConfig {
        origin_url: server.origin_url(),
        application_token: "application-token".to_string(),
        p2p: P2pConfig::default(),
    })
    .expect("create SDK client");
    let mut progress = Vec::new();

    let result = client
        .sync_object_with_summary_and_progress(
            SyncObjectRequest {
                bucket: "game-assets".to_string(),
                key: "maps/desert-v3.pak".to_string(),
                destination: destination.clone(),
            },
            Some(
                &mut |fragment_index, bytes_downloaded, total_bytes, source_type| {
                    progress.push((
                        fragment_index,
                        bytes_downloaded,
                        total_bytes,
                        source_type.to_string(),
                    ));
                },
            ),
        )
        .expect("sync object through local Ponte Mesh protocol");

    assert_eq!(
        std::fs::read(destination).expect("read destination"),
        object_bytes()
    );
    assert_eq!(progress.len(), 2);
    assert!(progress
        .iter()
        .all(|(_, _, _, source_type)| source_type == "REPLICA_EDGE"));
    assert_eq!(
        result.summary.bytes_from_replica,
        object_bytes().len() as u64
    );
    assert_eq!(result.summary.fragments_from_replica, 2);
    assert_eq!(result.summary.bytes_from_origin, 0);
    assert_eq!(result.summary.fragments_from_origin, 0);
    assert_eq!(result.summary.fallback_activations, 0);

    let requests = server.requests();
    assert!(requests
        .iter()
        .any(|request| request.method == "POST" && request.target == "/pontemesh/access-packages"));
    assert!(requests.iter().any(|request| {
        request.method == "GET"
            && request
                .target
                .starts_with("/pontemesh/objects/game-assets/manifest/")
    }));
    assert_eq!(
        requests
            .iter()
            .filter(|request| request
                .target
                .starts_with("/pontemesh/replica/access-packages/"))
            .count(),
        2
    );
    assert_event(&requests, "FRAGMENT_VALIDATED");
    assert_event_field(&requests, r#""fragmentIndex":0"#);
    assert_event_field(&requests, r#""fragmentHash":"#);
    assert_event_field(&requests, r#""bytesTransferred":"#);
    assert_event_field(&requests, r#""outcome":"SUCCESS""#);
    assert_all_targets_hide_package_token(&requests);
    assert_all_bodies_hide_tokens(&requests);
    assert_authorization_seen(
        &requests,
        "/pontemesh/access-packages",
        "Bearer application-token",
    );
    assert!(requests.iter().any(|request| {
        request
            .target
            .starts_with("/pontemesh/replica/access-packages/")
            && header(request, "authorization") == Some("Bearer package-token-secret")
            && header(request, "range").is_some()
    }));
}

#[test]
fn sdk_falls_back_from_replica_to_origin_and_records_source_failure() {
    let server = TestServer::start(true);
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let destination = temp_dir.path().join("maps/desert-v3.pak");
    let client = PontemeshClient::new(PontemeshClientConfig {
        origin_url: server.origin_url(),
        application_token: "application-token".to_string(),
        p2p: P2pConfig::default(),
    })
    .expect("create SDK client");
    let mut progress_sources = Vec::new();

    let result = client
        .sync_object_with_summary_and_progress(
            SyncObjectRequest {
                bucket: "game-assets".to_string(),
                key: "maps/desert-v3.pak".to_string(),
                destination: destination.clone(),
            },
            Some(&mut |_, _, _, source_type| {
                progress_sources.push(source_type.to_string());
            }),
        )
        .expect("sync object with replica fallback");

    assert_eq!(
        std::fs::read(destination).expect("read destination"),
        object_bytes()
    );
    assert!(progress_sources.iter().all(|source| source == "ORIGIN"));
    assert_eq!(
        result.summary.bytes_from_origin,
        object_bytes().len() as u64
    );
    assert_eq!(result.summary.fragments_from_origin, 2);
    assert_eq!(result.summary.bytes_from_replica, 0);
    assert_eq!(result.summary.fragments_from_replica, 0);
    assert_eq!(result.summary.fallback_activations, 2);
    let requests = server.requests();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request
                .target
                .starts_with("/pontemesh/replica/access-packages/"))
            .count(),
        2
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| {
                request
                    .target
                    .starts_with("/pontemesh/access-packages/pkg-1/objects/")
            })
            .count(),
        2
    );
    assert_event(&requests, "SOURCE_FAILURE");
    assert_event(&requests, "FRAGMENT_VALIDATED");
    assert_all_targets_hide_package_token(&requests);
    assert_all_bodies_hide_tokens(&requests);
}

fn handle_connection(mut stream: TcpStream, addr: SocketAddr, state: &Arc<TestState>) {
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    state.requests.lock().unwrap().push(request.clone());

    if request.method == "POST" && request.target == "/pontemesh/access-packages" {
        write_json(&mut stream, 200, &access_package_json(addr));
        return;
    }
    if request.method == "GET"
        && request
            .target
            .starts_with("/pontemesh/objects/game-assets/manifest/")
    {
        write_json(&mut stream, 200, &manifest_json());
        return;
    }
    if request.method == "POST"
        && request
            .target
            .starts_with("/pontemesh/access-packages/pkg-1/events/")
    {
        if header(&request, "authorization") != Some("Bearer package-token-secret") {
            write_text(&mut stream, 401, "events require package token");
            return;
        }
        write_json(&mut stream, 202, r#"{"accepted":true}"#);
        return;
    }
    if request.method == "GET"
        && request
            .target
            .starts_with("/pontemesh/replica/access-packages/pkg-1/objects/")
    {
        if state.replica_should_fail {
            write_text(&mut stream, 503, "replica unavailable");
        } else {
            write_fragment(&mut stream, &request);
        }
        return;
    }
    if request.method == "GET"
        && request
            .target
            .starts_with("/pontemesh/access-packages/pkg-1/objects/")
    {
        write_fragment(&mut stream, &request);
        return;
    }

    write_text(&mut stream, 404, "not found");
}

fn read_request(stream: &mut TcpStream) -> Option<LoggedRequest> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set read timeout");
    let mut buffer = Vec::new();
    let mut scratch = [0; 1024];
    let header_end = loop {
        let read = stream.read(&mut scratch).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&scratch[..read]);
        if let Some(position) = find_header_end(&buffer) {
            break position;
        }
    };
    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or_default();
    while buffer.len() < header_end + 4 + content_length {
        let read = stream.read(&mut scratch).ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&scratch[..read]);
    }

    let mut lines = headers.lines();
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let parsed_headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    let body_start = header_end + 4;
    let body =
        String::from_utf8_lossy(&buffer[body_start..buffer.len().min(body_start + content_length)])
            .to_string();

    Some(LoggedRequest {
        method,
        target,
        headers: parsed_headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn access_package_json(addr: SocketAddr) -> String {
    let manifest = manifest_contract();
    let sources = vec![
        AuthorizedSource {
            id: "replica-1".to_string(),
            source_type: SourceType::ReplicaEdge,
            endpoint: format!(
                "http://{addr}/pontemesh/replica/access-packages/pkg-1/objects/game-assets/maps%2Fdesert-v3.pak"
            ),
            peer_id: None,
            transport: None,
            priority: 1,
            expires_at: "2099-01-01T00:00:00Z".to_string(),
            available_fragments: vec![0, 1],
        },
        AuthorizedSource {
            id: "origin-1".to_string(),
            source_type: SourceType::Origin,
            endpoint: format!(
                "http://{addr}/pontemesh/access-packages/pkg-1/objects/game-assets/maps%2Fdesert-v3.pak"
            ),
            peer_id: None,
            transport: None,
            priority: 2,
            expires_at: "2099-01-01T00:00:00Z".to_string(),
            available_fragments: vec![0, 1],
        },
    ];
    serde_json::to_string(&AccessPackage {
        id: "pkg-1".to_string(),
        package_token: "package-token-secret".to_string(),
        bucket: manifest.bucket.clone(),
        key: manifest.key.clone(),
        version: manifest.version.clone(),
        manifest_id: manifest.manifest_id.clone(),
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        scope: vec!["object:read".to_string()],
        authorized_sources: sources,
        source_selection: SourceSelectionContract {
            strategy: "PEER_REPLICA_ORIGIN".to_string(),
            fragment_priority: "MANIFEST_ORDER".to_string(),
            failure_threshold: 2,
            allow_peer_sharing: true,
            allow_replica_edge: true,
        },
        fallback: FallbackContract {
            source_type: "ORIGIN".to_string(),
            object_endpoint: format!(
                "http://{addr}/pontemesh/access-packages/pkg-1/objects/game-assets/maps%2Fdesert-v3.pak"
            ),
            supports_range: true,
            preserve_validated_fragments: true,
            mode: "RANGE".to_string(),
            revalidate_endpoint: None,
        },
        manifest,
    })
    .expect("serialize access package")
}

fn manifest_json() -> String {
    serde_json::to_string(&manifest_contract()).expect("serialize manifest")
}

fn manifest_contract() -> Manifest {
    let object = object_bytes();
    let first = &object[..10];
    let second = &object[10..];
    Manifest {
        manifest_id: "manifest-1".to_string(),
        object_id: "object-1".to_string(),
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        version: "v1".to_string(),
        total_size_bytes: object.len() as i64,
        content_type: "application/octet-stream".to_string(),
        object_hash_algorithm: "SHA256".to_string(),
        object_sha256: sha256_hex(&object),
        fragment_size_bytes: 10,
        fragments: vec![
            FragmentDescriptor {
                index: 0,
                fragment_id: "fragment-0".to_string(),
                byte_range_start: 0,
                byte_range_end: 9,
                size_bytes: first.len(),
                hash_algorithm: "SHA256".to_string(),
                sha256: sha256_hex(first),
                priority: "NORMAL".to_string(),
                fallback_range_header: "bytes=0-9".to_string(),
            },
            FragmentDescriptor {
                index: 1,
                fragment_id: "fragment-1".to_string(),
                byte_range_start: 10,
                byte_range_end: object.len() as u64 - 1,
                size_bytes: second.len(),
                hash_algorithm: "SHA256".to_string(),
                sha256: sha256_hex(second),
                priority: "NORMAL".to_string(),
                fallback_range_header: format!("bytes=10-{}", object.len() - 1),
            },
        ],
        availability_state: "AVAILABLE".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn write_fragment(stream: &mut TcpStream, request: &LoggedRequest) {
    let object = object_bytes();
    let (start, end) = parse_range(header(request, "range").expect("range header"));
    let body = &object[start..=end];
    write_response(
        stream,
        206,
        "application/octet-stream",
        body,
        &[(
            "Content-Range",
            &format!("bytes {start}-{end}/{}", object.len()),
        )],
    );
}

fn parse_range(value: &str) -> (usize, usize) {
    let value = value.strip_prefix("bytes=").expect("bytes range");
    let (start, end) = value.split_once('-').expect("range separator");
    (
        start.parse().expect("range start"),
        end.parse().expect("range end"),
    )
}

fn write_json(stream: &mut TcpStream, status: u16, body: &str) {
    write_response(stream, status, "application/json", body.as_bytes(), &[]);
}

fn write_text(stream: &mut TcpStream, status: u16, body: &str) {
    write_response(stream, status, "text/plain", body.as_bytes(), &[]);
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    extra_headers: &[(&str, &str)],
) {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        206 => "Partial Content",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in extra_headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    stream
        .write_all(response.as_bytes())
        .expect("write headers");
    stream.write_all(body).expect("write body");
}

fn header<'a>(request: &'a LoggedRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn assert_event(requests: &[LoggedRequest], event_type: &str) {
    assert!(
        requests.iter().any(|request| {
            request.method == "POST"
                && request
                    .target
                    .starts_with("/pontemesh/access-packages/pkg-1/events/")
                && request.body.contains(event_type)
        }),
        "missing SDK event {event_type}"
    );
}

fn assert_event_field(requests: &[LoggedRequest], field: &str) {
    assert!(
        requests.iter().any(|request| {
            request.method == "POST"
                && request
                    .target
                    .starts_with("/pontemesh/access-packages/pkg-1/events/")
                && request.body.contains(field)
        }),
        "missing SDK event field {field}"
    );
}

fn assert_all_targets_hide_package_token(requests: &[LoggedRequest]) {
    for request in requests {
        assert!(
            !request.target.contains("package-token-secret"),
            "package token leaked into URL target {}",
            request.target
        );
    }
}

fn assert_all_bodies_hide_tokens(requests: &[LoggedRequest]) {
    for request in requests {
        assert!(
            !request.body.contains("package-token-secret"),
            "package token leaked into request body for {}",
            request.target
        );
        assert!(
            !request.body.contains("application-token"),
            "application token leaked into request body for {}",
            request.target
        );
    }
}

fn assert_authorization_seen(requests: &[LoggedRequest], target: &str, expected: &str) {
    assert!(
        requests.iter().any(|request| {
            request.target == target && header(request, "authorization") == Some(expected)
        }),
        "missing authorization header for {target}"
    );
}

fn object_bytes() -> Vec<u8> {
    b"desert-map-native-sdk".to_vec()
}
