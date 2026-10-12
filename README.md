<div align="center">

<img src="design/assets/living-ring.webp" width="360" alt="The Omnarc living ring: every arc is a device, gold comets are tokens travelling through them">

# Omnarc

**Every device is an arc. Together, one mind.**

Omnarc turns computers, phones, servers and browsers into one AI cluster —<br>
from the devices in one home to idle devices around the world.

[![CI](https://github.com/HumoyunDavronov2001/omnarc/actions/workflows/ci.yml/badge.svg)](https://github.com/HumoyunDavronov2001/omnarc/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-3dd7cf)](LICENSE)
![Status: pre-alpha, stage B1 in progress](https://img.shields.io/badge/status-pre--alpha%20%C2%B7%20B1%20in%20progress-f2c156)

[Plan](PLAN.md) · [Requirements](REQUIREMENTS.md) · [Design concept](design/concept.html) · [Protocol](protocol/README.md) · [Contributing](CONTRIBUTING.md)

</div>

> *Omnarc* = *omni* (all) + *arc*: every device is an arc, and together they close one ring.

## Why

Billions of devices sit idle most of the day. Together they hold more compute than any data center, yet today's
tools each cover only one corner of it:

| | exo | prima.cpp | GPUStack | **Omnarc** (goal) |
|---|:-:|:-:|:-:|:-:|
| Phones: Android, iOS | — | Termux only | — | native apps |
| Browsers: join with a QR code, no install | — | — | — | ✓ |
| Windows · Linux · macOS | macOS | Linux, macOS | Linux | ✓ |
| CUDA · ROCm/Vulkan · Metal · OpenCL · NPU | Metal | CUDA | server GPUs | ✓ |
| Devices join and leave at runtime | ✓ | — | ✓ | ✓ |
| Across the internet, behind NAT | — | — | — | ✓ |
| Inference **and** training | — | — | — | ✓ |

The Omnarc column lists goals of the [roadmap](#roadmap), not shipped features.

## What it is for

- **Agents on your own model.** OpenClaw, OpenCode and other agents run on a frontier-class open model spread over
  your own computers — no per-token bill and no rate limit. One computer can only stream such a model from disk; a
  cluster keeps all of it in memory.
- **The family ring.** A family or a team shares one assistant that speaks their language; a weak phone joins by voice
  and computes nothing.
- **Accuracy mode.** Several devices answer the same question in parallel and check each other, and the answer shows
  how many of them agreed.

These are goals of the [roadmap](#roadmap), stages B4, B7 and B8.

## How it works

- **Every device is an arc on the ring.** A model's layers are split across devices in proportion to their measured
  power, and a token travels around the ring through each of them.
- **More devices make a single answer faster, too.** A continuous pipeline with speculative decoding keeps every
  device busy at the same time instead of waiting for its turn.
- **Adding a device never slows you down.** A model that fits on one device gets whole copies on the others, not
  slices; layers are split only when a model does not fit.
- **Only what is active is computed.** MoE experts and hot/cold neurons are spread across devices, so
  trillion-parameter models fit on ordinary hardware.
- **Nothing stops when a device leaves.** Layer replicas and continuously copied KV caches take over within a second.
- **Any device, any link.** Native apps on five platforms plus a WebGPU browser node; QUIC with NAT traversal over
  Wi-Fi, Ethernet, USB-C and Thunderbolt.
- **Private by default.** All traffic is encrypted. On the public network raw text never leaves your device, and every
  answer carries a TOPLOC proof.

```mermaid
flowchart LR
  U["App / user<br/>OpenAI · Claude · Ollama API"] --> G
  subgraph N["omnarc-node (on every device, Rust)"]
    G["API gateway"] --> P["Plan executor"]
    P --> E["Engine host"]
    T["Network: iroh QUIC<br/>gossip · Raft"] --- P
    S["Model store<br/>iroh-blobs, BLAKE3"] --- E
  end
  E <-->|"shared memory"| L["libomnarc (C++)<br/>llama.cpp + patches"]
  N <-->|"QUIC, encrypted"| N2["other nodes"]
  B["Browser node<br/>WASM + WebGPU"] <-->|"WebRTC"| N
```

## Design

<picture>
  <source media="(prefers-color-scheme: light)" srcset="design/assets/home-screen-light.png">
  <img src="design/assets/home-screen-dark.png" alt="The Ring home screen: six devices as arcs around the ring, the token path below and the device list on the right">
</picture>

The interface shows the mechanism itself: an arc's length is the device's share of layers, its color is the load,
and tokens move at the measured speed — so a slow device or a slow link stands out at a glance.

- Colors come from Samarkand's tilework — lapis, turquoise, gold and ganch plaster — with a single accent.
- One token source ([`design/colors.py`](design/colors.py)) generates CSS, Swift, Kotlin and TypeScript; every text
  color passes WCAG AA and every chart color passes color-vision checks.
- The living ring ([`design/living-ring.js`](design/living-ring.js), WebGL2, no dependencies) is the brand mark:
  ~15,000 GPU particles, one comet per token in flight.

See the full [design concept](design/concept.html) (download it and open it in a browser) and the
[living ring video](design/living-ring.mp4).

## Status

**Pre-alpha.** Stage B0 (foundation) is complete; stage B1 (engine) is in progress. Devices can already run one
model together as a ring wired by hand; discovery, pairing and planning come in stages B2 and B3.

Stage B1 so far:

- [`engine/`](engine) — libomnarc on llama.cpp: a model splits into **windows of layers** that run as separate
  shards, with hidden states passed between them ([`engine/patches`](engine/patches/README.md)). The accuracy gate,
  `omnarc-gate`, checks every split against the whole model: on the CPU the split run is bit for bit the whole run,
  and with the hidden state sent as `f16` the perplexity stays within 0.2%.
- The engine runs in **its own process**, driven by the agent through shared memory
  ([`engine/HOST.md`](engine/HOST.md), [`omnarc-engine-host`](node/crates/omnarc-engine-host)).
- **The ring** — prima.cpp's piped ring on Omnarc's engine and network
  ([`omnarc-node ring`](node/crates/omnarc-node/src/ring.rs), [`omnarc-network`](node/crates/omnarc-network)): the
  device that serves the request keeps the first layers and the output head, the hidden state travels around the
  ring over QUIC (iroh), and a ring of two nodes generates token for token what one device generates.

Stage B0 delivered:

- [`protocol/`](protocol) — `omnarc.v1` messages and the tensor frame spec, with a Rust implementation and property tests.
- [`node/`](node) — the Rust workspace; `omnarc-node info` describes a device the way it will announce itself.
- [`engine/`](engine) — the engine's C ABI, tested from plain C.
- [`design/`](design) — design tokens for every platform with a contrast gate, the living ring, the design concept.
- [`web/`](web) — the Svelte 5 component library in Storybook, with visual-regression and axe (WCAG 2.2 AA) checks.
- CI on Linux, Windows, macOS, Android, iOS and the web.

## Roadmap

| Stage | Scope | Gate |
|---|---|---|
| **B0 Foundation** ✓ done | monorepo, CI, protocol v1, design system | CI green on 6 platforms; visual and accessibility checks |
| **B1 Engine** ← in progress | llama.cpp + prima.cpp piped-ring, Windows, Vulkan | a 70B model across Linux and Windows machines |
| B2 Network | iroh P2P, QR pairing, peer-to-peer model distribution | two networks behind NAT form one cluster |
| B3 Scheduler | placement, live join and leave, replicas | recovery within 1 s; adding a device never slows a request |
| B4 Speed and accuracy | pipelined speculation, MoE, sparse FFN, accuracy mode | single-request speed grows with the device count; 8 devices answer more accurately than one |
| B5 Devices | desktop, Android, iOS, browser, NPU | six node types; joining from a browser in 30 s |
| B6 Scale | organizations, public network, credits | 1,000+ nodes |
| B7 Modalities and training | speech, images, LoRA, RL, the family ring | distributed LoRA over the internet; spoken answers on a weak phone |
| B8 Islands and API | MLX, vLLM/SGLang, OpenAI/Claude/Ollama APIs, agent backend | throughput on par with vLLM on NVIDIA; agents on a frontier-class open model |
| B9 World benchmark | open benchmarks against exo, prima.cpp, Parallax, Mesh LLM, colibri and others | v1.0 |

Details: [PLAN.md](PLAN.md) and [REQUIREMENTS.md](REQUIREMENTS.md).

## Build

```bash
# Protocol and node agent (Rust)
cargo test --workspace
cargo run -p omnarc-node -- info

# Engine (C++20 on llama.cpp, a git submodule; the tests need Python with numpy)
git submodule update --init
cmake -S engine -B build/engine && cmake --build build/engine && ctest --test-dir build/engine

# Design tokens and the concept page
python3 design/colors.py && python3 design/build.py

# Component library (inside web/)
pnpm install && pnpm check && pnpm test && pnpm build-storybook && pnpm test:visual
```

With [just](https://github.com/casey/just) installed, `just check` runs all of it.

Run a ring of two devices (stage B1 wires it by hand; `omnarc-engine` must sit next to `omnarc-node`):

```bash
omnarc-node id    # on each device: its node id

# the worker: layers 40–79 of an 80-layer model
omnarc-node ring --model model.gguf --layers 40:80 --listen 0.0.0.0:7700 \
  --next <head id>@<head ip>:7700 --prev <head id>

# the head: layers 0–39 and the output head (layer 80); it generates and prints the time per token
omnarc-node ring --model model.gguf --layers 0:40,80:81 --listen 0.0.0.0:7700 \
  --next <worker id>@<worker ip>:7700 --prev <worker id> --prompt "Explain how a ring of devices runs one model." --generate 64
```

## Repository

```
protocol/   Protobuf messages (omnarc.v1) and the tensor frame spec
node/       Rust workspace: omnarc-protocol, omnarc-engine-host, omnarc-network, omnarc-node
engine/     libomnarc — C++20 engine on llama.cpp with a C ABI
design/     design tokens, the living ring, the design concept
web/        Svelte 5 component library (Storybook)
docs/uz/    Uzbek reading copy of the plan
```

## Credits

Omnarc builds on [llama.cpp](https://github.com/ggml-org/llama.cpp), [prima.cpp](https://github.com/OpenCPIL/prima.cpp),
[exo](https://github.com/exo-explore/exo), [GPUStack](https://github.com/gpustack/gpustack) and other open-source work —
see [UPSTREAM.md](UPSTREAM.md) and [NOTICE](NOTICE).

## License

[Apache License 2.0](LICENSE)
