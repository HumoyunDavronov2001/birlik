#!/usr/bin/env python3
"""Writes a tiny random-weight GGUF model of a supported architecture for the engine tests.

The weights are random, so the model says nothing useful, but it runs exactly the graph of its architecture:
comparing a model split into windows with the same model run whole checks the window code path end to end.
The writer is llama.cpp's gguf-py (pass --gguf-py with its directory, e.g. engine/third_party/llama.cpp/gguf-py).
"""

import argparse
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=["llama", "qwen3"], required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--gguf-py", type=Path, required=True, help="llama.cpp's gguf-py directory")
    parser.add_argument("--layers", type=int, default=8)
    parser.add_argument("--seed", type=int, default=1)
    args = parser.parse_args()

    sys.path.insert(0, str(args.gguf_py))
    import gguf  # noqa: E402  (from the path above)
    import numpy as np  # noqa: E402

    n_layer, n_embd, n_head, n_head_kv, n_ff, n_vocab = args.layers, 64, 4, 2, 160, 256
    head_dim = n_embd // n_head
    rng = np.random.default_rng(args.seed)

    def matrix(rows: int, cols: int) -> "np.ndarray":
        # numpy shape (rows, cols) is the ggml tensor {cols, rows}: a matmul from cols to rows
        return (rng.standard_normal((rows, cols)) / np.sqrt(cols)).astype(np.float32)

    def norm(size: int) -> "np.ndarray":
        return (1.0 + 0.1 * rng.standard_normal(size)).astype(np.float32)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    writer = gguf.GGUFWriter(str(args.out), args.arch)
    writer.add_block_count(n_layer)
    writer.add_context_length(512)
    writer.add_embedding_length(n_embd)
    writer.add_feed_forward_length(n_ff)
    writer.add_head_count(n_head)
    writer.add_head_count_kv(n_head_kv)
    writer.add_key_length(head_dim)
    writer.add_value_length(head_dim)
    writer.add_rope_dimension_count(head_dim)
    writer.add_rope_freq_base(10000.0)
    writer.add_layer_norm_rms_eps(1e-6)
    writer.add_vocab_size(n_vocab)
    writer.add_tokenizer_model("no_vocab")
    writer.add_file_type(gguf.LlamaFileType.ALL_F32)

    writer.add_tensor("token_embd.weight", rng.standard_normal((n_vocab, n_embd)).astype(np.float32))
    if args.arch == "llama":
        # Llama 3 keeps one rope frequency table for all layers; every window must find it, not only the first
        writer.add_tensor("rope_freqs.weight", (1.0 + rng.random(head_dim // 2)).astype(np.float32))
    for i in range(n_layer):
        writer.add_tensor(f"blk.{i}.attn_norm.weight", norm(n_embd))
        writer.add_tensor(f"blk.{i}.attn_q.weight", matrix(n_head * head_dim, n_embd))
        writer.add_tensor(f"blk.{i}.attn_k.weight", matrix(n_head_kv * head_dim, n_embd))
        writer.add_tensor(f"blk.{i}.attn_v.weight", matrix(n_head_kv * head_dim, n_embd))
        writer.add_tensor(f"blk.{i}.attn_output.weight", matrix(n_embd, n_head * head_dim))
        if args.arch == "qwen3":
            writer.add_tensor(f"blk.{i}.attn_q_norm.weight", norm(head_dim))
            writer.add_tensor(f"blk.{i}.attn_k_norm.weight", norm(head_dim))
        writer.add_tensor(f"blk.{i}.ffn_norm.weight", norm(n_embd))
        writer.add_tensor(f"blk.{i}.ffn_gate.weight", matrix(n_ff, n_embd))
        writer.add_tensor(f"blk.{i}.ffn_up.weight", matrix(n_ff, n_embd))
        writer.add_tensor(f"blk.{i}.ffn_down.weight", matrix(n_embd, n_ff))
    writer.add_tensor("output_norm.weight", norm(n_embd))
    writer.add_tensor("output.weight", matrix(n_vocab, n_embd))

    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_tensors_to_file()
    writer.close()
    print(f"wrote {args.out} ({args.arch}, {n_layer} layers)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
