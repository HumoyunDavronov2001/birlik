//! The control API of an Omnarc node (`protocol/CLUSTER.md` §5): `ClusterService` over the Connect protocol, and the
//! panel's files, on one HTTP port that by default only the device itself can reach.
//!
//! The node implements [`Cluster`]; [`router`] turns it into an axum router. This crate speaks the part of the
//! Connect protocol the panel and the apps use: unary calls and server streams, with binary Protobuf messages
//! (`application/proto`, `application/connect+proto`) and Connect's JSON error bodies.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use futures_util::stream::{BoxStream, StreamExt as _};
use omnarc_protocol::v1;
use prost::Message;

/// The path prefix of the service's procedures.
pub const SERVICE: &str = "/omnarc.v1.ClusterService";

/// The largest request body the API reads.
const MAX_REQUEST: usize = 1 << 20;

/// A Connect error code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    InvalidArgument,
    NotFound,
    FailedPrecondition,
    PermissionDenied,
    Unimplemented,
    Internal,
}

impl Code {
    const fn name(self) -> &'static str {
        match self {
            Self::InvalidArgument => "invalid_argument",
            Self::NotFound => "not_found",
            Self::FailedPrecondition => "failed_precondition",
            Self::PermissionDenied => "permission_denied",
            Self::Unimplemented => "unimplemented",
            Self::Internal => "internal",
        }
    }

    const fn status(self) -> StatusCode {
        match self {
            Self::InvalidArgument | Self::FailedPrecondition => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::PermissionDenied => StatusCode::FORBIDDEN,
            Self::Unimplemented => StatusCode::NOT_IMPLEMENTED,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// An error of a call, sent as Connect's JSON error body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: Code,
    pub message: String,
}

impl Error {
    /// An error with `code` and a message for people.
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "code": self.code.name(), "message": self.message })
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let mut response = (self.code.status(), self.json().to_string()).into_response();
        response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
        response
    }
}

/// What the API asks of the node.
pub trait Cluster: Send + Sync + 'static {
    /// The cluster as this node knows it now.
    fn snapshot(&self) -> v1::ClusterSnapshot;

    /// What happens from now on; the API sends a snapshot first.
    fn watch(&self) -> BoxStream<'static, v1::WatchClusterResponse>;

    /// A one-time ticket that adds a device, valid for `ttl_seconds` (the node caps it at 10 minutes).
    ///
    /// # Errors
    /// The node cannot admit devices.
    fn create_ticket(&self, ttl_seconds: u32) -> Result<v1::CreateTicketResponse, Error>;

    /// The owner's answer for a device that asked to join; whether the device was still waiting.
    ///
    /// # Errors
    /// The request names no device.
    fn answer_join(&self, request: &v1::AnswerJoinRequest) -> Result<bool, Error>;
}

/// The API's routes for `cluster`, and the panel's files from `panel` when given.
pub fn router(cluster: Arc<dyn Cluster>, panel: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route(&format!("{SERVICE}/GetCluster"), post(get_cluster))
        .route(&format!("{SERVICE}/WatchCluster"), post(watch_cluster))
        .route(&format!("{SERVICE}/CreateTicket"), post(create_ticket))
        .route(&format!("{SERVICE}/AnswerJoin"), post(answer_join))
        .with_state(cluster);
    match panel {
        Some(dir) => {
            let index = dir.join("index.html");
            api.fallback_service(
                tower_http::services::ServeDir::new(dir).fallback(tower_http::services::ServeFile::new(index)),
            )
        }
        None => api,
    }
}

type ClusterState = State<Arc<dyn Cluster>>;

async fn get_cluster(State(cluster): ClusterState, headers: HeaderMap, body: Bytes) -> Response {
    unary(&headers, &body, |_: v1::GetClusterRequest| Ok(v1::GetClusterResponse { cluster: Some(cluster.snapshot()) }))
}

async fn create_ticket(State(cluster): ClusterState, headers: HeaderMap, body: Bytes) -> Response {
    unary(&headers, &body, |request: v1::CreateTicketRequest| cluster.create_ticket(request.ttl_seconds))
}

async fn answer_join(State(cluster): ClusterState, headers: HeaderMap, body: Bytes) -> Response {
    unary(&headers, &body, |request: v1::AnswerJoinRequest| {
        cluster.answer_join(&request).map(|waited| v1::AnswerJoinResponse { waited })
    })
}

// A unary call: one binary Protobuf message in, one out.
fn unary<Req: Message + Default, Resp: Message>(
    headers: &HeaderMap,
    body: &[u8],
    call: impl FnOnce(Req) -> Result<Resp, Error>,
) -> Response {
    if !content_type_is(headers, "application/proto") {
        return Error::new(Code::Unimplemented, "this API speaks Connect with binary messages (application/proto)")
            .into_response();
    }
    if body.len() > MAX_REQUEST {
        return Error::new(Code::InvalidArgument, "the request is too large").into_response();
    }
    let request = match Req::decode(body) {
        Ok(request) => request,
        Err(e) => return Error::new(Code::InvalidArgument, format!("the request is malformed: {e}")).into_response(),
    };
    match call(request) {
        Ok(response) => {
            let mut response = (StatusCode::OK, response.encode_to_vec()).into_response();
            response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/proto"));
            response
        }
        Err(error) => error.into_response(),
    }
}

async fn watch_cluster(State(cluster): ClusterState, headers: HeaderMap, body: Bytes) -> Response {
    if !content_type_is(&headers, "application/connect+proto") {
        return Error::new(
            Code::Unimplemented,
            "a stream speaks Connect with binary messages (application/connect+proto)",
        )
        .into_response();
    }
    if let Err(error) = open_envelope::<v1::WatchClusterRequest>(&body) {
        return error.into_response();
    }
    let first =
        v1::WatchClusterResponse { event: Some(v1::watch_cluster_response::Event::Snapshot(cluster.snapshot())) };
    let events = futures_util::stream::once(async move { first })
        .chain(cluster.watch())
        .map(|event| Ok::<Bytes, std::convert::Infallible>(envelope(0, &event.encode_to_vec())));
    let mut response = Response::new(Body::from_stream(events));
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/connect+proto"));
    response
}

fn content_type_is(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(';').next().is_some_and(|kind| kind.trim().eq_ignore_ascii_case(expected)))
}

/// A Connect envelope: flags, the length as 32-bit big-endian, and the bytes.
#[must_use]
pub fn envelope(flags: u8, message: &[u8]) -> Bytes {
    let mut bytes = Vec::with_capacity(5 + message.len());
    bytes.push(flags);
    bytes.extend_from_slice(&u32::try_from(message.len()).unwrap_or(u32::MAX).to_be_bytes());
    bytes.extend_from_slice(message);
    Bytes::from(bytes)
}

// The single message of an enveloped request body.
fn open_envelope<M: Message + Default>(body: &[u8]) -> Result<M, Error> {
    if body.len() < 5 || body.len() > MAX_REQUEST {
        return Err(Error::new(Code::InvalidArgument, "the request is not one Connect envelope"));
    }
    let len = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
    if body[0] != 0 || body.len() != 5 + len {
        return Err(Error::new(Code::InvalidArgument, "the request is not one uncompressed Connect envelope"));
    }
    M::decode(&body[5..]).map_err(|e| Error::new(Code::InvalidArgument, format!("the request is malformed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    struct Fake;

    impl Cluster for Fake {
        fn snapshot(&self) -> v1::ClusterSnapshot {
            v1::ClusterSnapshot { name: "Home cluster".into(), can_invite: true, ..Default::default() }
        }

        fn watch(&self) -> BoxStream<'static, v1::WatchClusterResponse> {
            let telemetry = v1::WatchClusterResponse {
                event: Some(v1::watch_cluster_response::Event::Telemetry(v1::Telemetry {
                    cpu_load: 0.25,
                    ..Default::default()
                })),
            };
            futures_util::stream::iter([telemetry]).chain(futures_util::stream::pending()).boxed()
        }

        fn create_ticket(&self, ttl_seconds: u32) -> Result<v1::CreateTicketResponse, Error> {
            if ttl_seconds == 0 {
                return Err(Error::new(Code::InvalidArgument, "a ticket lives at least a second"));
            }
            Ok(v1::CreateTicketResponse { qr_payload: "OMNARC:ABC".into(), ..Default::default() })
        }

        fn answer_join(&self, request: &v1::AnswerJoinRequest) -> Result<bool, Error> {
            Ok(request.matches)
        }
    }

    fn app() -> Router {
        router(Arc::new(Fake), None)
    }

    fn call(path: &str, content_type: &str, body: Vec<u8>) -> Request<Body> {
        Request::post(format!("{SERVICE}/{path}"))
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn a_unary_call_answers_in_binary() {
        let response = app()
            .oneshot(call("GetCluster", "application/proto", v1::GetClusterRequest {}.encode_to_vec()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/proto");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let cluster = v1::GetClusterResponse::decode(body).unwrap().cluster.unwrap();
        assert_eq!(cluster.name, "Home cluster");
        assert!(cluster.can_invite);
    }

    #[tokio::test]
    async fn an_error_is_connect_json() {
        let request = v1::CreateTicketRequest { ttl_seconds: 0 }.encode_to_vec();
        let response = app().oneshot(call("CreateTicket", "application/proto", request)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(body["code"], "invalid_argument");

        let json = app().oneshot(call("GetCluster", "application/json", b"{}".to_vec())).await.unwrap();
        assert_eq!(json.status(), StatusCode::NOT_IMPLEMENTED, "only binary messages");
        let garbage = app().oneshot(call("GetCluster", "application/proto", vec![0xff; 3])).await.unwrap();
        assert_eq!(garbage.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_stream_starts_with_a_snapshot() {
        let request = envelope(0, &v1::WatchClusterRequest {}.encode_to_vec()).to_vec();
        let response = app().oneshot(call("WatchCluster", "application/connect+proto", request)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        let mut buffer = Vec::new();
        let mut events = Vec::new();
        while events.len() < 2 {
            let frame = body.frame().await.unwrap().unwrap();
            buffer.extend_from_slice(&frame.into_data().unwrap());
            while buffer.len() >= 5 {
                let len = u32::from_be_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]) as usize;
                if buffer.len() < 5 + len {
                    break;
                }
                assert_eq!(buffer[0], 0, "a message, not the end of the stream");
                events.push(v1::WatchClusterResponse::decode(&buffer[5..5 + len]).unwrap());
                buffer.drain(..5 + len);
            }
        }
        assert!(matches!(
            &events[0].event,
            Some(v1::watch_cluster_response::Event::Snapshot(s)) if s.name == "Home cluster"
        ));
        assert!(matches!(
            &events[1].event,
            Some(v1::watch_cluster_response::Event::Telemetry(t)) if (t.cpu_load - 0.25).abs() < 1e-6
        ));
    }

    #[tokio::test]
    async fn a_malformed_stream_request_is_refused() {
        let response = app().oneshot(call("WatchCluster", "application/connect+proto", vec![0, 0, 0])).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
