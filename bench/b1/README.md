# Stage B1 gate

PLAN.md §11 closes stage B1 when three things hold:

1. **Accuracy gate** on Llama 3.x 8B, Qwen3 32B and R1-Distill 70B: every split of a model matches the model run
   whole — mean KL divergence ≤ 0.01 and perplexity within 0.5% (PLAN.md §4).
2. **A 70B model runs on Linux and Windows computers** as one ring.
3. **Time per output token (TPOT) is no worse than prima.cpp's** on the same hardware.

The models, all Q4_K_M from Hugging Face, are listed in [`models.txt`](models.txt) with their sizes and SHA-256
(67 GB together). [`download.sh`](download.sh) (Linux, macOS) and [`download.ps1`](download.ps1) (Windows) fetch
them; a broken download resumes, and every file is checked before use. Results go to [`results/`](results/) with the
host, the backend and the commits they were measured on.

```bash
bench/b1/download.sh ~/models            # all three; or 8b, 32b, 70b
```
```powershell
powershell -ExecutionPolicy Bypass -File bench\b1\download.ps1 -Dir D:\models -Model 70b
```

Ready-made programs for each system (`birlik-node`, `birlik-engine`, `birlik-gate`) are the artifacts of every CI run
on `main` (Actions → CI → a run → Artifacts); unpack them into one folder.

## 1. Accuracy gate

```bash
just engine                                  # build/engine/birlik-gate
bench/b1/gate.sh ~/models                    # every model, three splits, wikitext-2, hidden state as f16
bench/b1/gate.sh ~/models cuda               # the same on a GPU backend, by name
```

The gate needs no second device: it runs the whole model and the windows one after another on one machine. Weights
that do not fit in memory are read from disk (memory-mapped), so a 70B model runs on a small machine too, only
slowly.

## 2. A 70B model on Linux and Windows

Two computers on one network, the model file on both. `birlik-engine` sits next to `birlik-node` (from the engine and
Rust builds, or a release). On each computer, `birlik-node id` prints its node id.

```bash
# Windows (the worker): layers 40–79
birlik-node ring --model R1-Distill-70B.gguf --layers 40:80 --listen 0.0.0.0:7700 \
  --next <linux id>@<linux ip>:7700 --prev <linux id>

# Linux (the head): layers 0–39 and the output head, layer 80
birlik-node ring --model R1-Distill-70B.gguf --layers 0:40,80:81 --listen 0.0.0.0:7700 \
  --next <windows id>@<windows ip>:7700 --prev <windows id> \
  --prompt "Explain how a ring of devices runs one model." --generate 128
```

The head prints the answer, the time to the first token and the time per output token. Windows Firewall must let
`birlik-node` receive UDP on port 7700 (it asks the first time the program listens).

## 3. TPOT against prima.cpp

prima.cpp runs on Linux and macOS but not on Windows, so this comparison uses two Linux (or macOS) computers. Both
systems get the same model, the same split, the same prompt and greedy decoding; each run generates 128 tokens and
is repeated three times, and the median TPOT counts.

| Split of an 80-layer model | prima.cpp (`llama-cli`, both devices) | BIRLIK (`birlik-node ring`) |
|---|---|---|
| one round: 40 + 40 layers | `--world 2 -lw "40,40" --prefetch` | head `0:40,80:81`, worker `40:80` |
| two rounds: 20 + 20 layers each | `--world 2 -lw "20,20" --prefetch` | head `0:20,40:60,80:81`, worker `20:40,60:80` |

prima.cpp, rank 0 and rank 1 — [`prima.sh`](prima.sh) builds it at the commit BIRLIK studied (it needs ZeroMQ:
`sudo apt install libzmq3-dev`; HiGHS is built by the script):

```bash
./llama-cli -m R1-Distill-70B.gguf -c 1024 -n 128 --temp 0 -p "<prompt>" \
  --world 2 --rank 0 --master <ip 0> --next <ip 1> --prefetch -lw "40,40"
./llama-cli -m R1-Distill-70B.gguf --world 2 --rank 1 --master <ip 0> --next <ip 0> --prefetch
```

prima.cpp reports `eval time … ms per token`; BIRLIK reports `ms per output token`. Record both in
`results/tpot-<date>.md` with the hardware of both computers.

## Results

| Gate | Status |
|---|---|
| Accuracy: tiny llama and qwen3 models, stories15M — every split, CPU, Metal, Vulkan (lavapipe) | passes in CI |
| Accuracy: Llama 3.x 8B, Qwen3 32B, R1-Distill 70B | to measure |
| 70B on Linux + Windows | to measure |
| TPOT ≤ prima.cpp | to measure |
