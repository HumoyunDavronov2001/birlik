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
(§1). Every message is a `SignedGossip`: the payload (a `Telemetry`, a `DeviceInfo` update or a leave notice), the
sender's `NodeId`, a timestamp and an Ed25519 signature by the sender's node key, so a member relaying a message cannot
change it. Telemetry goes out about once per second.

### 3.2 Raft

The agreed state — members and their rights, the revocation list, the cluster name and settings, and from stage B3
the placement plans — is a log replicated with [openraft](https://github.com/databendlabs/openraft) on the ALPN
`omnarc/raft/1`. Only computers and servers vote, 3 to 5 of them; phones and browsers follow the state through gossip.
A cluster of one or two computers runs Raft with that many voters.

### 3.3 Links

Every device measures the round-trip time of each connection to another member every second and the bandwidth with a
short probe once a minute and whenever a link changes. The samples travel in `Telemetry.links`.

## 4. Spreading a model

1. One device downloads the GGUF file from Hugging Face, hashes it with BLAKE3 and adds it to its
   [iroh-blobs](https://github.com/n0-computer/iroh-blobs) store. The internet is used once.
2. Each device computes from the GGUF header the byte ranges of the tensors its layers need (the header and metadata,
   its layers' tensors, and for the stage holding layer 0 or the output head also the embedding and output tensors).
3. It fetches only those ranges from any member that has them; iroh-blobs verifies every chunk against the hash, and
   a broken download resumes where it stopped.
4. The ranges are written into a sparse file of the full size, so the engine maps the same GGUF file as everywhere
   else and never touches the ranges it does not have.

## 5. The control API

Every node serves `ClusterService` (cluster_service.proto) over Connect RPC and the panel's files on
`http://127.0.0.1:7600`. Only the device itself can reach it by default; serving it to the LAN is a setting.
