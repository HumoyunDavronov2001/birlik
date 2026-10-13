//! A model downloaded once from a source that speaks like Hugging Face: a redirect with the file's SHA-256, then the
//! file by ranges — fetched, resumed, checked, and refused when it does not match.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use omnarc_node::download;

struct Source {
    file: Vec<u8>,
    sha256: String,
    served: AtomicU64,
}

async fn resolve(State(source): State<Arc<Source>>) -> Response {
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, "/cdn/model.gguf".to_owned()),
            (header::HeaderName::from_static("x-linked-etag"), format!("\"{}\"", source.sha256)),
            (header::HeaderName::from_static("x-linked-size"), source.file.len().to_string()),
        ],
    )
        .into_response()
}

async fn cdn(State(source): State<Arc<Source>>, headers: HeaderMap) -> Response {
    let from = headers
        .get(header::RANGE)
        .and_then(|range| range.to_str().ok())
        .and_then(|range| range.strip_prefix("bytes=")?.strip_suffix('-')?.parse::<usize>().ok());
    let body = source.file[from.unwrap_or(0)..].to_vec();
    source.served.fetch_add(body.len() as u64, Ordering::Relaxed);
    let status = if from.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK };
    (status, body).into_response()
}

async fn source(file: Vec<u8>, sha256: String) -> (String, Arc<Source>) {
    use std::future::IntoFuture as _;
    let state = Arc::new(Source { file, sha256, served: AtomicU64::new(0) });
    let app = Router::new()
        .route("/org/model/resolve/main/model.gguf", get(resolve).head(resolve))
        .route("/cdn/model.gguf", get(cdn))
        .with_state(Arc::clone(&state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());
    (url, state)
}

fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    ring::digest::digest(&ring::digest::SHA256, bytes).as_ref().iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[tokio::test]
async fn a_model_is_fetched_once_resumed_and_checked() {
    let dir = std::env::temp_dir().join(format!("omnarc-download-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
    let (url, state) = source(file.clone(), sha256(&file)).await;

    // a third arrived before the connection broke
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.gguf.part"), &file[..1_000_000]).unwrap();
    let pulled = download::pull(&url, "org/model", "model.gguf", &dir, |_, _| {}).await.unwrap();
    assert!(pulled.verified);
    assert_eq!(pulled.bytes, 3_000_000);
    assert_eq!(std::fs::read(&pulled.path).unwrap(), file);
    assert_eq!(state.served.load(Ordering::Relaxed), 2_000_000, "only the rest travelled");
    assert!(!dir.join("model.gguf.part").exists());

    // already there: only checked
    download::pull(&url, "org/model", "model.gguf", &dir, |_, _| {}).await.unwrap();
    assert_eq!(state.served.load(Ordering::Relaxed), 2_000_000);

    // a file that does not match what the source published is refused and deleted
    let (bad, _) = source(file.clone(), "0".repeat(64)).await;
    let other = dir.join("other");
    let refused = download::pull(&bad, "org/model", "model.gguf", &other, |_, _| {}).await;
    assert!(refused.unwrap_err().to_string().contains("does not match"));
    assert!(!other.join("model.gguf").exists());

    std::fs::remove_dir_all(dir).unwrap();
}
