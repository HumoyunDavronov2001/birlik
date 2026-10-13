//! The first run (PLAN.md §9.6, flow 1): `omnarc-node serve` on a device that belongs to no cluster yet serves the
//! panel and an API that can only create a cluster or join one. Once either is done, the service starts as usual.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::BoxFuture;
use futures_util::stream::{BoxStream, StreamExt as _};
use omnarc_api::{Code, Error};
use omnarc_network::ticket::Ticket;
use omnarc_protocol::v1;
use tokio::sync::{mpsc, watch};

use crate::pairing;

/// The API of a device in no cluster.
#[derive(Debug)]
pub struct Setup {
    data: PathBuf,
    device: v1::DeviceInfo,
    me: v1::NodeId,
    done: watch::Sender<bool>,
}

impl Setup {
    /// The setup of the node in `data`, which describes itself as `device`.
    ///
    /// # Errors
    /// The node's key cannot be read or created.
    pub fn new(data: &Path, device: v1::DeviceInfo) -> Result<Arc<Self>, crate::ring::BoxError> {
        let key = omnarc_network::load_or_create_key(&data.join("node.key"))?;
        let me = v1::NodeId { ed25519_public_key: key.public().as_bytes().to_vec() };
        let device = v1::DeviceInfo { id: Some(me.clone()), ..device };
        Ok(Arc::new(Self { data: data.to_path_buf(), device, me, done: watch::Sender::new(false) }))
    }

    /// Resolves once a cluster was created or joined.
    pub async fn done(&self) {
        let mut done = self.done.subscribe();
        let _ = done.wait_for(|done| *done).await;
    }
}

fn error(e: impl std::fmt::Display) -> Error {
    let message = e.to_string();
    let code = if message.contains("already belongs") { Code::FailedPrecondition } else { Code::InvalidArgument };
    Error::new(code, message)
}

impl omnarc_api::Cluster for Setup {
    fn snapshot(&self) -> v1::ClusterSnapshot {
        v1::ClusterSnapshot {
            this_node: Some(self.me.clone()),
            devices: vec![self.device.clone()],
            ..Default::default()
        }
    }

    // nothing happens until a cluster exists; then the stream ends and the panel reconnects to the service
    fn watch(&self) -> BoxStream<'static, v1::WatchClusterResponse> {
        let done = self.done.subscribe();
        futures_util::stream::unfold(done, |mut done| async move {
            let _ = done.wait_for(|done| *done).await;
            None
        })
        .boxed()
    }

    fn create_ticket(&self, _ttl_seconds: u32) -> Result<v1::CreateTicketResponse, Error> {
        Err(Error::new(Code::FailedPrecondition, "this device belongs to no cluster yet"))
    }

    fn answer_join(&self, _request: &v1::AnswerJoinRequest) -> Result<bool, Error> {
        Err(Error::new(Code::FailedPrecondition, "this device belongs to no cluster yet"))
    }

    fn remove_member(&self, _member: v1::NodeId) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async { Err(Error::new(Code::FailedPrecondition, "this device belongs to no cluster yet")) })
    }

    fn create_cluster(&self, request: v1::CreateClusterRequest) -> BoxFuture<'static, Result<(), Error>> {
        let (data, device, done) = (self.data.clone(), self.device.clone(), self.done.clone());
        Box::pin(async move {
            let name = if request.name.trim().is_empty() { "Home cluster" } else { request.name.trim() };
            pairing::create(&data, name, device, &request.relays).map_err(error)?;
            done.send_replace(true);
            Ok(())
        })
    }

    fn join_cluster(
        &self,
        request: v1::JoinClusterRequest,
    ) -> BoxStream<'static, Result<v1::JoinClusterResponse, Error>> {
        use v1::join_cluster_response::Step;
        let (data, mut device, done) = (self.data.clone(), self.device.clone(), self.done.clone());
        if !request.device_name.trim().is_empty() {
            request.device_name.trim().clone_into(&mut device.name);
        }
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            let ticket = match Ticket::decode(request.ticket.trim()) {
                Ok(ticket) => ticket,
                Err(e) => {
                    let _ = tx.send(Err(error(e))).await;
                    return;
                }
            };
            let early = tx.clone();
            let joined = pairing::join(&data, &ticket, device, move |emoji| {
                let shown = v1::SasSymbols {
                    symbols: emoji
                        .iter()
                        .map(|e| v1::SasSymbol { emoji: e.symbol.into(), name: e.name.into() })
                        .collect(),
                };
                let _ = early.try_send(Ok(v1::JoinClusterResponse { step: Some(Step::Symbols(shown)) }));
            })
            .await;
            let last = match joined {
                Ok(cluster) => Ok(v1::JoinClusterResponse { step: Some(Step::Joined(cluster.name)) }),
                Err(e) => Err(error(e)),
            };
            let ok = last.is_ok();
            let _ = tx.send(last).await;
            if ok {
                done.send_replace(true);
            }
        });
        futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|step| (step, rx)) }).boxed()
    }
}
