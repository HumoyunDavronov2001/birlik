//! Downloading a model once, from Hugging Face (PLAN.md §3.8): the file is fetched where the cluster keeps its
//! downloads, resumed after a break, and checked against the SHA-256 Hugging Face publishes for it. The cluster then
//! spreads it peer to peer; the internet is used once.

use std::path::{Path, PathBuf};

use n0_future::StreamExt as _;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::ring::BoxError;

/// Where models come from unless told otherwise.
pub const HUGGING_FACE: &str = "https://huggingface.co";

/// A downloaded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pulled {
    pub path: PathBuf,
    pub bytes: u64,
    /// The file's SHA-256, in lowercase hex.
    pub sha256: String,
    /// Whether it matched the hash the source published; `false` when the source published none.
    pub verified: bool,
}

/// What the source says about a file before it is downloaded: its size and SHA-256, when it gives them.
#[derive(Debug, Default)]
struct Published {
    size: Option<u64>,
    sha256: Option<String>,
}

fn client(redirects: bool) -> Result<reqwest::Client, BoxError> {
    // the TLS of the node: ring, as iroh uses it
    let _ = rustls::crypto::ring::default_provider().install_default();
    let policy = if redirects { reqwest::redirect::Policy::limited(10) } else { reqwest::redirect::Policy::none() };
    Ok(reqwest::Client::builder()
        .user_agent(concat!("omnarc-node/", env!("CARGO_PKG_VERSION")))
        .redirect(policy)
        .build()?)
}

/// Hugging Face answers a file of large-file storage with a redirect that carries `X-Linked-Etag` (its SHA-256) and
/// `X-Linked-Size`.
async fn published(url: &str) -> Result<Published, BoxError> {
    let response = client(false)?.head(url).send().await?;
    if response.status().is_client_error() {
        return Err(format!("{url}: {}", response.status()).into());
    }
    let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let sha256 = header("x-linked-etag")
        .map(|etag| etag.trim_matches('"').trim_start_matches("W/").trim_matches('"').to_ascii_lowercase())
        .filter(|etag| etag.len() == 64 && etag.bytes().all(|b| b.is_ascii_hexdigit()));
    let size = header("x-linked-size").and_then(|size| size.parse().ok());
    Ok(Published { size, sha256 })
}

/// The SHA-256 of the file at `path`.
async fn sha256_of(path: &Path) -> Result<String, BoxError> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = vec![0u8; 8 << 20];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        context.update(&buffer[..n]);
    }
    Ok(context.finish().as_ref().iter().fold(String::with_capacity(64), |mut out, b| {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
        out
    }))
}

/// Downloads `file` of the repository `repo` from `source` (such as [`HUGGING_FACE`]) into `dir`. A download that
/// broke off resumes; a file already there is only checked. `progress` hears the bytes there are and the size.
///
/// # Errors
/// The file cannot be fetched or written, or it does not match the SHA-256 the source published — then it is
/// deleted.
pub async fn pull(
    source: &str,
    repo: &str,
    file: &str,
    dir: &Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<Pulled, BoxError> {
    let url = format!("{}/{repo}/resolve/main/{file}", source.trim_end_matches('/'));
    let name = Path::new(file).file_name().ok_or_else(|| format!("{file} names no file"))?;
    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join(name);
    let part = dir.join(format!("{}.part", name.to_string_lossy()));
    let published = published(&url).await?;

    let have = tokio::fs::metadata(&path).await.map(|m| m.len()).ok();
    if have.is_none() || published.size.is_some_and(|size| Some(size) != have) {
        let mut at = tokio::fs::metadata(&part).await.map_or(0, |m| m.len());
        let mut request = client(true)?.get(&url);
        if at > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={at}-"));
        }
        let response = request.send().await?.error_for_status()?;
        // a source that ignores the range sends the whole file again
        let resumed = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        if !resumed {
            at = 0;
        }
        let size = published.size.or_else(|| response.content_length().map(|len| len + at));
        let mut out = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(&part)
            .await?;
        let mut body = response.bytes_stream();
        progress(at, size);
        while let Some(chunk) = body.next().await {
            let chunk = chunk?;
            out.write_all(&chunk).await?;
            at += chunk.len() as u64;
            progress(at, size);
        }
        out.sync_all().await?;
        if let Some(size) = size.filter(|&size| size != at) {
            return Err(format!("the download stopped at {at} of {size} bytes; run it again to resume").into());
        }
        tokio::fs::rename(&part, &path).await?;
    }

    let bytes = tokio::fs::metadata(&path).await?.len();
    let sha256 = sha256_of(&path).await?;
    if let Some(expected) = &published.sha256
        && *expected != sha256
    {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(format!("{file} does not match its published SHA-256 ({sha256}, not {expected}); deleted").into());
    }
    Ok(Pulled { path, bytes, sha256, verified: published.sha256.is_some() })
}
