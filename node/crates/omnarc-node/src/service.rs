//! The agent's long-running service (`omnarc-node serve`): one endpoint for the cluster's protocols
//! (`protocol/CLUSTER.md`).
//!
//! The service keeps the set of members a node lets in, admits devices through the invitations its owner opens, and
//! talks on the cluster's gossip topic: its `DeviceInfo` when it starts, telemetry once per interval, and every
//! member an admin admits. What it hears it keeps in memory and passes on as [`Event`]s, which the control API and
//! the panel stream.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use n0_future::StreamExt as _;
use omnarc_network::cluster::{self, ClusterKey};
use omnarc_network::gossip::{
    self, GOSSIP_ALPN, Gossip, GossipEvent, GossipReceiver, GossipSender, Members, MembersOnly,
};
use omnarc_network::join::{self, ClusterContext, Invitation, JOIN_ALPN, JoinAttempt};
use omnarc_network::protocol::{AcceptError, ProtocolHandler};
use omnarc_network::ticket::Ticket;
use omnarc_network::{Connection, Endpoint, EndpointAddr, EndpointId, MemoryLookup, Router, SecretKey, TransportAddr};
use omnarc_protocol::v1;
use tokio::sync::{broadcast, oneshot};

use crate::pairing::now_unix_ms;
use crate::ring::BoxError;
use crate::state::{Cluster, Member, Store};

/// What the service reports as it happens.
#[derive(Clone, Debug)]
pub enum Event {
    /// A member's telemetry.
    Telemetry(EndpointId, v1::Telemetry),
    /// A member told what it is.
    Device(EndpointId, v1::DeviceInfo),
    /// A device was admitted, by this node or by another admin.
    MemberJoined(EndpointId),
    /// A member left on purpose.
    MemberLeft(EndpointId),
    /// A direct gossip neighbor came up or went down.
    Neighbor { id: EndpointId, up: bool },
    /// A device asks to join and waits for the owner to compare the symbols: answer with [`Service::answer_join`].
    JoinAttempt(JoinAttempt),
}

/// How the service runs.
#[derive(Clone, Debug)]
pub struct Config {
    /// The node's data directory.
    pub data: PathBuf,
    /// Where the node listens.
    pub listen: SocketAddr,
    /// What this device is; its id and addresses are filled in.
    pub device: v1::DeviceInfo,
    /// How often the node sends its telemetry.
    pub telemetry_every: Duration,
}

/// A running service.
#[derive(Debug)]
pub struct Service {
    shared: Arc<Shared>,
    router: Router,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
struct Shared {
    key: SecretKey,
    cluster: Cluster,
    cluster_key: Option<ClusterKey>,
    store: Store,
    members: Members,
    lookup: MemoryLookup,
    endpoint: Endpoint,
    sender: GossipSender,
    events: broadcast::Sender<Event>,
    telemetry: Mutex<HashMap<EndpointId, v1::Telemetry>>,
    devices: Mutex<HashMap<EndpointId, v1::DeviceInfo>>,
    invitation: Mutex<Option<Arc<Invitation>>>,
    answers: Mutex<HashMap<EndpointId, oneshot::Sender<bool>>>,
    // wakes the speaker to announce this device again, e.g. when a neighbor comes up
    announce: tokio::sync::Notify,
}

/// Telemetry ticks between two announcements of the device even when nothing changed.
const ANNOUNCE_EVERY: u32 = 60;

impl Service {
    /// Starts the service of the node in `config.data`, which must belong to a cluster.
    ///
    /// # Errors
    /// The node belongs to no cluster, its state cannot be read, or the endpoint cannot be bound.
    ///
    /// # Panics
    /// Never: the join handler gets the shared state once.
    pub async fn start(config: Config) -> Result<Self, BoxError> {
        let store = Store::open(&config.data)?;
        let cluster =
            store.cluster()?.ok_or("this device belongs to no cluster yet: run omnarc-node create or join")?;
        let cluster_key = store.cluster_key()?;
        let key = omnarc_network::load_or_create_key(&config.data.join("node.key"))?;
        let known = store.members()?;
        let members = Members::new(known.iter().map(|member| member.id));
        let lookup = MemoryLookup::new();
        for member in known.iter().filter(|member| member.id != key.public() && !member.addrs.is_empty()) {
            lookup.add_endpoint_info(addr_of(member.id, &member.addrs));
        }

        let endpoint = omnarc_network::bind_cluster(key.clone(), config.listen, lookup.clone()).await?;
        let gossip = Gossip::builder().spawn(endpoint.clone());
        let joins = JoinHandler(Arc::new(OnceLock::new()));
        let router = Router::builder(endpoint.clone())
            .accept(GOSSIP_ALPN, MembersOnly::new(gossip.clone(), members.clone()))
            .accept(JOIN_ALPN, joins.clone())
            .spawn();

        let others: Vec<EndpointId> = members.list().into_iter().filter(|id| *id != key.public()).collect();
        let (sender, receiver) = gossip.subscribe(gossip::topic(cluster.id), others).await?.split();
        let (events, _) = broadcast::channel(1024);
        let shared = Arc::new(Shared {
            key,
            cluster,
            cluster_key,
            store,
            members,
            lookup,
            endpoint,
            sender,
            events,
            telemetry: Mutex::default(),
            devices: Mutex::default(),
            invitation: Mutex::default(),
            answers: Mutex::default(),
            announce: tokio::sync::Notify::new(),
        });
        joins.0.set(Arc::clone(&shared)).expect("set once");

        // the name the owner gave this device when it joined or created the cluster stays its name
        let mut device = config.device;
        if let Some(me) = known.iter().find(|member| member.id == shared.key.public())
            && !me.device.name.is_empty()
        {
            device.name.clone_from(&me.device.name);
        }
        device.id = Some(node_id(shared.key.public()));
        let tasks = vec![
            tokio::spawn(listen(Arc::clone(&shared), receiver)),
            tokio::spawn(speak(Arc::clone(&shared), device, config.telemetry_every)),
        ];
        Ok(Self { shared, router, tasks })
    }

    /// This node's id.
    #[must_use]
    pub fn id(&self) -> EndpointId {
        self.shared.key.public()
    }

    /// The cluster the node belongs to.
    #[must_use]
    pub fn cluster(&self) -> &Cluster {
        &self.shared.cluster
    }

    /// The addresses this node can be dialed at.
    #[must_use]
    pub fn addrs(&self) -> Vec<SocketAddr> {
        own_addrs(&self.shared.endpoint)
    }

    /// The members this node lets in.
    #[must_use]
    pub fn members(&self) -> Vec<EndpointId> {
        self.shared.members.list()
    }

    /// The latest telemetry heard from each member.
    ///
    /// # Panics
    /// When a task panicked while holding the telemetry.
    #[must_use]
    pub fn telemetry(&self) -> HashMap<EndpointId, v1::Telemetry> {
        self.shared.telemetry.lock().expect("telemetry").clone()
    }

    /// What each member said it is.
    ///
    /// # Panics
    /// When a task panicked while holding the devices.
    #[must_use]
    pub fn devices(&self) -> HashMap<EndpointId, v1::DeviceInfo> {
        self.shared.devices.lock().expect("devices").clone()
    }

    /// Events from now on.
    #[must_use]
    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.shared.events.subscribe()
    }

    /// Opens an invitation for one device, valid for `ttl` (at most 10 minutes); it replaces an earlier one.
    ///
    /// # Errors
    /// This node holds no cluster key.
    ///
    /// # Panics
    /// When a task panicked while holding the invitation.
    pub fn invite(&self, ttl: Duration, relays: Vec<String>) -> Result<Ticket, BoxError> {
        if self.shared.cluster_key.is_none() {
            return Err("this device holds no cluster key: invite from the device that created the cluster".into());
        }
        let ticket = Ticket::new(
            self.shared.cluster.id,
            self.id(),
            own_addrs(&self.shared.endpoint),
            relays,
            ttl,
            now_unix_ms(),
        )?;
        *self.shared.invitation.lock().expect("invitation") = Some(Arc::new(Invitation::new(ticket.clone())));
        Ok(ticket)
    }

    /// The owner's answer for a device that asked to join: whether the symbols match. Whether such a device waited.
    ///
    /// # Panics
    /// When a task panicked while holding the answers.
    #[must_use = "whether a device was waiting for this answer"]
    pub fn answer_join(&self, joiner: EndpointId, matches: bool) -> bool {
        let waiting = self.shared.answers.lock().expect("answers").remove(&joiner);
        waiting.is_some_and(|answer| answer.send(matches).is_ok())
    }

    /// The control API's view of this service.
    #[must_use]
    pub fn api(&self) -> Arc<dyn omnarc_api::Cluster> {
        Arc::new(ApiView(Arc::clone(&self.shared)))
    }

    /// Tells the cluster that this device leaves, then stops.
    pub async fn shutdown(self) {
        let leave = v1::GossipPayload { kind: Some(v1::gossip_payload::Kind::Leave(v1::LeaveNotice {})) };
        let _ = self.shared.sender.broadcast(gossip::seal(&self.shared.key, &leave, now_unix_ms()).into()).await;
        for task in &self.tasks {
            task.abort();
        }
        let _ = self.router.shutdown().await;
    }
}

// What the control API sees of the service.
struct ApiView(Arc<Shared>);

impl ApiView {
    fn device(&self, id: EndpointId) -> v1::DeviceInfo {
        let known = self.0.devices.lock().expect("devices").get(&id).cloned();
        known.unwrap_or_else(|| v1::DeviceInfo { id: Some(node_id(id)), ..Default::default() })
    }
}

impl omnarc_api::Cluster for ApiView {
    fn snapshot(&self) -> v1::ClusterSnapshot {
        let shared = &self.0;
        v1::ClusterSnapshot {
            id: Some(v1::ClusterId { ed25519_public_key: shared.cluster.id.as_bytes().to_vec() }),
            name: shared.cluster.name.clone(),
            devices: shared.members.list().into_iter().map(|id| self.device(id)).collect(),
            telemetry: shared.telemetry.lock().expect("telemetry").values().cloned().collect(),
            plans: vec![],
            this_node: Some(node_id(shared.key.public())),
            can_invite: shared.cluster_key.is_some(),
        }
    }

    fn watch(&self) -> n0_future::boxed::BoxStream<v1::WatchClusterResponse> {
        use v1::watch_cluster_response::Event as Out;
        let view = ApiView(Arc::clone(&self.0));
        let events = self.0.events.subscribe();
        Box::pin(n0_future::stream::unfold((view, events), |(view, mut events)| async move {
            loop {
                let event = match events.recv().await {
                    Ok(Event::Telemetry(_, telemetry)) => Out::Telemetry(telemetry),
                    Ok(Event::Device(_, device)) => Out::DeviceJoined(device),
                    Ok(Event::MemberJoined(id)) => Out::DeviceJoined(view.device(id)),
                    Ok(Event::MemberLeft(id)) => Out::DeviceLeft(node_id(id)),
                    Ok(Event::JoinAttempt(attempt)) => Out::JoinAttempt(v1::JoinAttempt {
                        joiner: Some(node_id(attempt.joiner)),
                        device: Some(attempt.device),
                        symbols: attempt
                            .emoji
                            .iter()
                            .map(|e| v1::SasSymbol { emoji: e.symbol.into(), name: e.name.into() })
                            .collect(),
                    }),
                    Ok(Event::Neighbor { .. }) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                };
                return Some((v1::WatchClusterResponse { event: Some(event) }, (view, events)));
            }
        }))
    }

    fn create_ticket(&self, ttl_seconds: u32) -> Result<v1::CreateTicketResponse, omnarc_api::Error> {
        use omnarc_api::{Code, Error};
        let shared = &self.0;
        if shared.cluster_key.is_none() {
            return Err(Error::new(Code::FailedPrecondition, "this device holds no cluster key"));
        }
        let ttl = Duration::from_secs(u64::from(ttl_seconds.clamp(1, 600)));
        let ticket = Ticket::new(
            shared.cluster.id,
            shared.key.public(),
            own_addrs(&shared.endpoint),
            vec![],
            ttl,
            now_unix_ms(),
        )
        .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        *shared.invitation.lock().expect("invitation") = Some(Arc::new(Invitation::new(ticket.clone())));
        let qr_payload = ticket.encode();
        let qr_svg = qrcode::QrCode::new(qr_payload.as_bytes())
            .map(|code| {
                code.render::<qrcode::render::svg::Color<'_>>().min_dimensions(240, 240).quiet_zone(true).build()
            })
            .unwrap_or_default();
        Ok(v1::CreateTicketResponse { ticket: Some(ticket.to_proto()), qr_payload, qr_svg })
    }

    fn answer_join(&self, request: &v1::AnswerJoinRequest) -> Result<bool, omnarc_api::Error> {
        let joiner = request
            .joiner
            .as_ref()
            .and_then(|id| <[u8; 32]>::try_from(id.ed25519_public_key.as_slice()).ok())
            .and_then(|bytes| EndpointId::from_bytes(&bytes).ok())
            .ok_or_else(|| omnarc_api::Error::new(omnarc_api::Code::InvalidArgument, "no device named"))?;
        let waiting = self.0.answers.lock().expect("answers").remove(&joiner);
        Ok(waiting.is_some_and(|answer| answer.send(request.matches).is_ok()))
    }
}

fn node_id(id: EndpointId) -> v1::NodeId {
    v1::NodeId { ed25519_public_key: id.as_bytes().to_vec() }
}

fn addr_of(id: EndpointId, addrs: &[SocketAddr]) -> EndpointAddr {
    EndpointAddr::from_parts(id, addrs.iter().copied().map(TransportAddr::Ip))
}

fn own_addrs(endpoint: &Endpoint) -> Vec<SocketAddr> {
    endpoint.addr().ip_addrs().copied().filter(|addr| !addr.ip().is_unspecified()).collect()
}

// Sends this node's device once and its telemetry every `every`.
async fn speak(shared: Arc<Shared>, mut device: v1::DeviceInfo, every: Duration) {
    let mut system = sysinfo::System::new();
    let mut ticks = tokio::time::interval(every);
    let mut since_announced = 0u32;
    loop {
        let woken = tokio::select! {
            _ = ticks.tick() => false,
            () = shared.announce.notified() => true,
        };
        // the device goes out at the start, when its addresses change, when a neighbor comes up, and now and then
        let addrs: Vec<String> = own_addrs(&shared.endpoint).iter().map(ToString::to_string).collect();
        since_announced += 1;
        if woken
            || since_announced >= ANNOUNCE_EVERY
            || addrs != device.direct_addresses
            || !shared.devices.lock().expect("devices").contains_key(&shared.key.public())
        {
            since_announced = 0;
            device.direct_addresses = addrs;
            shared.devices.lock().expect("devices").insert(shared.key.public(), device.clone());
            let payload = v1::GossipPayload { kind: Some(v1::gossip_payload::Kind::Device(device.clone())) };
            let _ = shared.sender.broadcast(gossip::seal(&shared.key, &payload, now_unix_ms()).into()).await;
        }
        if woken {
            continue;
        }
        system.refresh_cpu_usage();
        system.refresh_memory();
        let telemetry = v1::Telemetry {
            node: Some(node_id(shared.key.public())),
            unix_ms: now_unix_ms(),
            cpu_load: system.global_cpu_usage() / 100.0,
            memory_used_bytes: system.used_memory(),
            ..Default::default()
        };
        shared.telemetry.lock().expect("telemetry").insert(shared.key.public(), telemetry.clone());
        let payload = v1::GossipPayload { kind: Some(v1::gossip_payload::Kind::Telemetry(telemetry)) };
        let _ = shared.sender.broadcast(gossip::seal(&shared.key, &payload, now_unix_ms()).into()).await;
    }
}

// Handles what the gossip topic brings.
async fn listen(shared: Arc<Shared>, mut receiver: GossipReceiver) {
    while let Some(event) = receiver.next().await {
        match event {
            Ok(GossipEvent::Received(message)) => {
                if let Ok(heard) = gossip::open(&message.content, &shared.members) {
                    hear(&shared, heard);
                }
            }
            Ok(GossipEvent::NeighborUp(id)) => {
                shared.announce.notify_one();
                let _ = shared.events.send(Event::Neighbor { id, up: true });
            }
            Ok(GossipEvent::NeighborDown(id)) => {
                let _ = shared.events.send(Event::Neighbor { id, up: false });
            }
            Ok(GossipEvent::Lagged) => {}
            Err(_) => break,
        }
    }
}

fn hear(shared: &Shared, heard: gossip::Heard) {
    use v1::gossip_payload::Kind;
    match heard.payload.kind {
        Some(Kind::Telemetry(telemetry)) => {
            shared.telemetry.lock().expect("telemetry").insert(heard.sender, telemetry.clone());
            let _ = shared.events.send(Event::Telemetry(heard.sender, telemetry));
        }
        Some(Kind::Device(device)) => {
            let addrs: Vec<SocketAddr> = device.direct_addresses.iter().filter_map(|addr| addr.parse().ok()).collect();
            if !addrs.is_empty() {
                shared.lookup.add_endpoint_info(addr_of(heard.sender, &addrs));
            }
            let _ = shared.store.update_member(heard.sender, Some(device.clone()), Some(addrs));
            shared.devices.lock().expect("devices").insert(heard.sender, device.clone());
            let _ = shared.events.send(Event::Device(heard.sender, device));
        }
        Some(Kind::Member(member)) => admit_announced(shared, &member),
        Some(Kind::Leave(_)) => {
            let _ = shared.events.send(Event::MemberLeft(heard.sender));
        }
        None => {}
    }
}

// A member announced by an admin: let it in once its token proves it.
fn admit_announced(shared: &Shared, member: &v1::Member) {
    let Some(id) = member.id.as_ref().and_then(|id| <[u8; 32]>::try_from(id.ed25519_public_key.as_slice()).ok()) else {
        return;
    };
    let Ok(id) = EndpointId::from_bytes(&id) else { return };
    if cluster::verify(shared.cluster.id, id, &member.membership_token, |_| false).is_err() {
        return;
    }
    let addrs: Vec<SocketAddr> = member.direct_addresses.iter().filter_map(|addr| addr.parse().ok()).collect();
    if !addrs.is_empty() {
        shared.lookup.add_endpoint_info(addr_of(id, &addrs));
    }
    let _ = shared.store.put_member(&Member {
        id,
        device: member.device.clone().unwrap_or_default(),
        token: member.membership_token.clone(),
        joined_unix_ms: member.joined_unix_ms,
        addrs,
    });
    if shared.members.insert(id) {
        let _ = shared.events.send(Event::MemberJoined(id));
    }
}

// The join protocol of the service: admits through the open invitation, asking the owner through events.
#[derive(Debug, Clone)]
struct JoinHandler(Arc<OnceLock<Arc<Shared>>>);

impl ProtocolHandler for JoinHandler {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let Some(shared) = self.0.get() else { return Ok(()) };
        let invitation = shared.invitation.lock().expect("invitation").clone();
        let (Some(invitation), Some(key)) = (invitation, shared.cluster_key.as_ref()) else {
            connection.close(404u32.into(), b"no open invitation");
            return Ok(());
        };
        let members = shared.members.list();
        let context = ClusterContext { key, name: &shared.cluster.name, members: &members };
        let ask = |attempt: JoinAttempt| {
            let (tx, rx) = oneshot::channel();
            shared.answers.lock().expect("answers").insert(attempt.joiner, tx);
            let _ = shared.events.send(Event::JoinAttempt(attempt));
            async move { rx.await.unwrap_or(false) }
        };
        let Ok(Some(admitted)) = join::admit(connection, &invitation, &context, now_unix_ms, ask).await else {
            return Ok(());
        };
        let member = v1::Member {
            id: Some(node_id(admitted.joiner)),
            device: Some(admitted.device.clone()),
            membership_token: admitted.token.clone(),
            joined_unix_ms: now_unix_ms(),
            direct_addresses: admitted.device.direct_addresses.clone(),
        };
        admit_announced(shared, &member);
        let announcement = v1::GossipPayload { kind: Some(v1::gossip_payload::Kind::Member(member)) };
        let _ = shared.sender.broadcast(gossip::seal(&shared.key, &announcement, now_unix_ms()).into()).await;
        Ok(())
    }
}
