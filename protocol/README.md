# Protocol

Everything Omnarc devices say to each other.

- `omnarc/v1/*.proto` — control messages (Protobuf), linted with [buf](https://buf.build) using the `STANDARD` rules.
  - `device.proto` — identity, platforms, backends, measured hardware.
  - `pairing.proto` — the QR ticket and the four-symbol (SAS) confirmation.
  - `cluster.proto` — a member of the cluster as every device records it.
  - `gossip.proto` — signed messages on the cluster's gossip topic.
  - `telemetry.proto` — load, heat, power and link measurements sent over gossip.
  - `plan.proto` — the placement plan agreed through Raft.
  - `cluster_service.proto` — the Connect RPC API served by every node.
- [`FRAME.md`](FRAME.md) — the binary frame that carries hidden states between pipeline stages.
- [`CLUSTER.md`](CLUSTER.md) — identities, adding a device, gossip, Raft and spreading a model (stage B2).

## Compatibility rules

- The package version (`omnarc.v1`) changes only for breaking changes; `buf breaking` guards it in CI.
- New fields are always optional, so an older node keeps working with a newer one.
- Field numbers are never reused.
