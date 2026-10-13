# Cluster protocol (stage B2)

How devices form one cluster, admit new devices, share state and spread models. The messages are in
[`omnarc/v1/*.proto`](omnarc/v1/); hidden states between stages travel in the frames of [`FRAME.md`](FRAME.md).

## 1. Identities

- **Node key.** Every device holds an Ed25519 key; its public key is the iroh `NodeId`. iroh's TLS handshake proves it
  on every connection, so a device never has to trust a claimed id.
- **Cluster key.** The device that creates a cluster generates a second Ed25519 key; its public key is the `ClusterId`.
  The private key stays on admin devices and signs membership tokens.
- **Membership token.** A [Biscuit](https://www.biscuitsec.org) token signed by the cluster key. Its authority block
  holds the facts

  ```
  cluster(hex:<cluster id>);
  node(hex:<node id>);
  right("node");          // and/or "user", "admin"
  issued(<unix seconds>);
  ```

  A device shows its token whenever it opens a cluster connection. The other side checks the signature against the
  `ClusterId`, checks that `node` equals the authenticated `NodeId` of the connection, and refuses a token whose
  revocation id is on the cluster's revocation list.

| Right | Allows |
|---|---|
| `node` | holding layers, gossip, model slices, Raft (computers and servers only) |
| `user` | the API: asking the cluster's models |
| `admin` | adding and removing devices and people, settings |

## 2. Adding a device

A device joins with a one-time ticket shown as a QR code on an admin device (or typed as a code, §2.3).

### 2.1 The ticket

`ClusterTicket` (pairing.proto): the cluster id, the inviting node, its direct addresses and relay URLs, 16 random
bytes `one_time_secret` and an expiry of at most 10 minutes. The QR code carries `OMNARC:` followed by the serialized
ticket in RFC 4648 base32 without padding. Uppercase letters and digits keep the QR code in its compact alphanumeric
mode; readers accept any case.

### 2.2 Joining

1. The new device dials the inviter by its `NodeId` (directly, through LAN discovery or through a relay from the
   ticket) on the ALPN `omnarc/join/1`, opens one bidirectional stream and sends a `JoinRequest`: its `DeviceInfo` and
   `secret_proof`, the keyed BLAKE3 hash of the joiner's `NodeId` under the key
   `BLAKE3-derive-key("omnarc/v1/join-proof", one_time_secret)`.
2. The inviter checks the expiry and the proof. A wrong proof counts as an attempt; after 3 attempts the ticket is
   void. A ticket admits exactly one device, and while one device waits for the owner's answer, others are refused.
3. Both devices derive the short authentication string:
   `BLAKE3("omnarc/v1/sas" ‖ inviter NodeId ‖ joiner NodeId ‖ one_time_secret)`; its first 24 bits are four 6-bit
   indices into the 64-emoji table (§2.4). Both screens show the four emoji.
4. The owner compares the screens and confirms on the inviter. On "No, they're different", or after 2 minutes, the
   inviter answers `accepted = false` and voids the ticket.
5. On confirmation the inviter signs a membership token with the `node` and `user` rights and answers
   `JoinResponse`: the token, the cluster's name and its current members, so the new device can reach them. The new
   device checks that the token is signed by the cluster of the ticket. The inviter announces the new member to the
   cluster (§3).

Messages on a join stream are protobuf, each prefixed by its length as an unsigned 32-bit little-endian integer, at
most 64 KiB.

### 2.3 A typed code

When a camera is not at hand, the inviter also shows an 8-digit code. The joiner finds inviters that have a code open
on the LAN (mDNS), and the code takes the place of `one_time_secret` in steps 1–3. The code is weak alone; the four
emoji make a guess or a man in the middle visible, and 3 wrong proofs void it.

### 2.4 The emoji table

The 64 emoji and their names are the table of the Matrix specification's SAS verification, chosen there to be easy to
tell apart and already translated into many languages: 🐶 Dog, 🐱 Cat, 🦁 Lion, 🐎 Horse, 🦄 Unicorn, 🐷 Pig,
🐘 Elephant, 🐰 Rabbit, 🐼 Panda, 🐓 Rooster, 🐧 Penguin, 🐢 Turtle, 🐟 Fish, 🐙 Octopus, 🦋 Butterfly, 🌷 Flower,
🌳 Tree, 🌵 Cactus, 🍄 Mushroom, 🌏 Globe, 🌙 Moon, ☁️ Cloud, 🔥 Fire, 🍌 Banana, 🍎 Apple, 🍓 Strawberry, 🌽 Corn,
🍕 Pizza, 🎂 Cake, ❤️ Heart, 😀 Smiley, 🤖 Robot, 🎩 Hat, 👓 Glasses, 🔧 Spanner, 🎅 Santa, 👍 Thumbs up,
☂️ Umbrella, ⌛ Hourglass, ⏰ Clock, 🎁 Gift, 💡 Light bulb, 📕 Book, ✏️ Pencil, 📎 Paperclip, ✂️ Scissors, 🔒 Lock,
🔑 Key, 🔨 Hammer, ☎️ Telephone, 🏁 Flag, 🚂 Train, 🚲 Bicycle, ✈️ Aeroplane, 🚀 Rocket, 🏆 Trophy, ⚽ Ball,
🎸 Guitar, 🎺 Trumpet, 🔔 Bell, ⚓ Anchor, 🎧 Headphones, 📁 Folder, 📌 Pin — index 0 to 63 in this order.

## 3. Shared state

### 3.1 Gossip

Members exchange telemetry and membership news over [iroh-gossip](https://github.com/n0-computer/iroh-gossip) on the
topic `BLAKE3("omnarc/v1/gossip" ‖ ClusterId)`. A gossip connection is accepted only from a node whose token is valid
(§1). Every message is a `SignedGossip`: the payload (a `Telemetry`, a `DeviceInfo` update, a new `Member` or a leave
notice), the sender's `NodeId`, a timestamp and an Ed25519 signature by the sender's node key, so a member relaying a
message cannot change it. Telemetry goes out about once per second — processor load, memory in use, a thermal state
from the device's hottest sensor, its power source and battery, and its link samples (§3.3); a device announces its
`DeviceInfo` when it starts and when it changes. The admin that admits a device announces it as a `Member` with its token, and every member checks
the token against the `ClusterId` before it lets the device connect. Gossip also spreads the addresses of the members,
so a new device that only knows its inviter finds the others.

### 3.2 Raft

The agreed state — the members and their tokens, the removed members and the revocation ids of their tokens, the
cluster's name and relays, and from stage B3 the placement plans — is a log replicated with
[openraft](https://github.com/databendlabs/openraft) on the ALPN `omnarc/raft/1` (members only). Each call is a short
stream carrying a length-prefixed JSON request and its answer; the log and the state are kept in `raft.redb` and
committed before Raft hears they are written.

- Only computers (Linux, Windows, macOS) keep the log; phones and browsers follow the state through gossip.
- The device that creates the cluster starts the log alone and writes into it what it knows: the name, the relays and
  the members.
- The leader adds every computer of the cluster as a learner and makes it a voter once its log keeps up, until five
  vote; a computer that has not caught up never stalls the vote.
- An admit is written into the log by the admin that admits; a change proposed on a follower goes to the leader.
- An admin removes a member: once the removal is committed, every computer lets the member in no more and refuses its
  token's revocation ids, also when the member is announced again.

Heartbeats go every 300 ms and an election starts after 1.5–3 s without one, since links reach across networks.

### 3.3 Links

Every device keeps a probe connection to each other member on the ALPN `omnarc/probe/1` (members only) and measures it:

- **Round-trip time**, every telemetry interval (one second): a stream carrying the byte `1`, echoed back.
- **Bandwidth**, once a minute and whenever the kind of link changes: a stream carrying the byte `2`, a length
  (u64, little-endian, at most 64 MiB) and that many bytes. The answer is the time between the first and the last byte
  arriving, in microseconds (u64), so opening the stream is not counted. A probe aims at about a quarter of a second:
  it grows fourfold while it takes under 200 ms and halves when it takes over a second.
- **Kind**: a relay path is `RELAY`; a direct path takes the kind of the local interface it leaves by — Wi-Fi,
  Ethernet, USB (a cable between two devices) or Thunderbolt.

The samples travel in `Telemetry.links`; a member that stops answering drops out of them within a few seconds.

### 3.4 Discovery

On the local network every running node announces itself by mDNS under the service name `omnarc`
(`<node id>._omnarc._udp.local`) with its direct addresses
([iroh-mdns-address-lookup](https://github.com/n0-computer/iroh-address-lookups), which uses
[swarm-discovery](https://crates.io/crates/swarm-discovery) to keep the traffic flat however many nodes there are). A
node dials a member by its id; when the addresses it knows are stale — a laptop got a new address from the router —
mDNS finds the member, and the node keeps the new addresses for the next time. The announcement shows only that an
Omnarc node runs there and its public id; whether a device may connect is still decided by its membership token
(§1). Across networks members are found through the addresses gossip spreads; DNS and pkarr records follow with the
relay.

### 3.5 Relays

A cluster may name relays — [iroh-relay](https://github.com/n0-computer/iroh) servers its owner runs
(`relay/README.md`) — through which members on different networks meet. Without one, members meet only on the networks
they share. A relay that admits only its owner's clusters wants a token; the cluster stores the relay's address with
its token as `https://relay.example.com/?token=SECRET`, and the token travels to the relay only in the `Authorization`
header of a relay connection.

- Every node connects to the nearest relay of the cluster, its home relay, and announces it in `DeviceInfo.relay_url`.
- A member is dialed at its direct addresses and its home relay; before it has announced one, at every relay of the
  cluster. Through the relay both sides learn each other's public addresses and punch a direct path; when punching
  fails, the relay carries the traffic, still encrypted end to end.
- A ticket names the cluster's relays with their tokens, so a device on another network reaches the inviter; once it
  joined, the cluster's relays are its own.

## 4. Spreading a model

1. One device downloads the GGUF file from Hugging Face and adds it to its
   [iroh-blobs](https://github.com/n0-computer/iroh-blobs) store, which hashes it with BLAKE3 and refers to the file
   where it lies instead of copying it. The internet is used once.
2. Another device fetches the header from it — 64 KiB first, doubling until the header is whole — and reads from it
   the byte ranges its window of layers needs. Windows count layers as the engine does: the repeating layers are
   `0 .. n_layer - 1` and the output layer is `n_layer`. A window needs the header, its layers' tensors (`blk.N.*`),
   the token embeddings when it starts at layer 0, the output norm and head when it ends after the last layer (and
   the token embeddings there too when the model has no head of its own), and every tensor that belongs to no layer,
   such as `rope_freqs.weight`.
3. It fetches only those ranges, on the iroh-blobs ALPN (`/iroh-bytes/4`), from any member that has them; a
   connection from a device that is not a member is closed. iroh-blobs checks every chunk against the BLAKE3 tree
   before it keeps it, and a fetch that breaks off resumes with the chunks still missing.
4. For the engine the ranges are written into a file of the model's full size with holes elsewhere, so the engine maps
   the same GGUF file as everywhere else and never reads the holes. The holes take no disk space on Linux and macOS;
   on Windows the file takes its full size until the desktop app marks it sparse.

## 5. The control API

Every node serves `ClusterService` (cluster_service.proto) over Connect RPC and the panel's files on
`http://127.0.0.1:7600`. Only the device itself can reach it by default; serving it to the LAN is a setting. Messages
are binary Protobuf (`application/proto`; streams `application/connect+proto`), errors Connect's JSON.

| Call | What it does |
|---|---|
| `GetCluster`, `WatchCluster` | the cluster as this node knows it, then every change: devices, telemetry, join attempts, removals |
| `CreateTicket`, `AnswerJoin` | an admin invites a device and answers its join attempt (§2) |
| `RemoveMember` | an admin removes a member once the cluster agrees (§3.2) |
| `CreateCluster`, `JoinCluster` | a device in no cluster yet creates one or joins one; `JoinCluster` streams the four symbols, then the cluster's name |

A node that belongs to no cluster yet answers only `GetCluster`, `WatchCluster`, `CreateCluster` and `JoinCluster`:
the panel's first run. Once the cluster exists, the service starts on the same address and the panel follows it. The
command line uses the same API while the service runs (`omnarc-node remove`).
