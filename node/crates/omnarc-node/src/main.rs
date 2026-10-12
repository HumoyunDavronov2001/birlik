//! `omnarc-node` — the agent that runs on every device.
//!
//! - `omnarc-node info` describes this device in the same shape (`omnarc.v1.DeviceInfo`) that it announces to a
//!   cluster.
//! - `omnarc-node id` prints this node's id, the public half of its key.
//! - `omnarc-node ring` runs windows of a model as one node of a ring of devices given by hand (stage B1).

use std::net::SocketAddr;
use std::path::PathBuf;

use omnarc_engine_host::{Backend, DType};
use omnarc_network::{EndpointId, Peer};
use omnarc_node::ring;
use omnarc_protocol::v1::{DeviceInfo, Platform};
use clap::{Parser, Subcommand};
use sysinfo::System;

#[derive(Debug, Parser)]
#[command(name = "omnarc-node", version, about = "The Omnarc agent that runs on every device")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Describe this device as it would announce itself to a cluster.
    Info {
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Print this node's id, creating its key if needed.
    Id {
        /// The key file (default: the user's data directory, omnarc/node.key).
        #[arg(long)]
        key: Option<PathBuf>,
    },
    /// Run windows of a model as one node of a ring. The node with a prompt is the head: it holds layer 0 and the
    /// output head and generates; the others serve until stopped.
    Ring(Box<RingArgs>),
}

#[derive(Debug, clap::Args)]
struct RingArgs {
    /// The GGUF model; every node of the ring uses the same file.
    #[arg(long)]
    model: PathBuf,
    /// This node's windows in ring order, e.g. `0:20,40:60,80:81`; layer N of an N-layer model is the output head.
    #[arg(long, value_delimiter = ',', required = true)]
    layers: Vec<ring::Window>,
    /// The next node of the ring: `<node id>@<ip:port>`.
    #[arg(long)]
    next: Peer,
    /// The id of the previous node; connections from other nodes are refused.
    #[arg(long)]
    prev: EndpointId,
    /// Where this node listens for the previous node.
    #[arg(long, default_value = "0.0.0.0:7700")]
    listen: SocketAddr,
    /// The prompt; given only on the head.
    #[arg(long)]
    prompt: Option<String>,
    /// Tokens to generate (head).
    #[arg(long, default_value_t = 64)]
    generate: u32,
    /// Backend for this node's layers: auto, cpu, cuda, hip, vulkan, metal, opencl, sycl.
    #[arg(long, default_value = "auto", value_parser = parse_backend)]
    backend: Backend,
    /// KV cache capacity in tokens.
    #[arg(long, default_value_t = 4096)]
    max_tokens: u32,
    /// Prompt tokens per prefill step.
    #[arg(long, default_value_t = 256)]
    chunk: u32,
    /// Element type of the hidden state between nodes: f16, bf16 or f32; the same on every node.
    #[arg(long, default_value = "f16", value_parser = parse_wire)]
    wire: DType,
    /// The key file (default: the user's data directory, omnarc/node.key).
    #[arg(long)]
    key: Option<PathBuf>,
    /// The omnarc-engine executable (default: next to omnarc-node).
    #[arg(long)]
    engine: Option<PathBuf>,
}

fn parse_backend(name: &str) -> Result<Backend, String> {
    Ok(match name {
        "auto" => Backend::Unspecified,
        "cpu" => Backend::Cpu,
        "cuda" => Backend::Cuda,
        "hip" => Backend::Hip,
        "vulkan" => Backend::Vulkan,
        "metal" => Backend::Metal,
        "opencl" => Backend::Opencl,
        "sycl" => Backend::Sycl,
        other => return Err(format!("unknown backend {other}")),
    })
}

fn parse_wire(name: &str) -> Result<DType, String> {
    match name {
        "f16" => Ok(DType::F16),
        "bf16" => Ok(DType::Bf16),
        "f32" => Ok(DType::F32),
        other => Err(format!("unknown wire format {other}; use f16, bf16 or f32")),
    }
}

fn ring(args: RingArgs) -> Result<(), ring::BoxError> {
    let config = ring::Config {
        model: args.model,
        windows: args.layers,
        listen: args.listen,
        next: args.next,
        prev: args.prev,
        key: args.key.unwrap_or_else(ring::default_key_path),
        engine: args.engine.unwrap_or_else(ring::default_engine_path),
        backend: args.backend,
        max_tokens: args.max_tokens,
        chunk: args.chunk.max(1),
        wire: args.wire,
        prompt: args.prompt,
        generate: args.generate,
    };
    let runtime = tokio::runtime::Runtime::new()?;
    if config.prompt.is_none() {
        return runtime.block_on(ring::serve(config));
    }
    let report = runtime.block_on(ring::generate(config))?;
    eprintln!(
        "omnarc: {} prompt tokens, {} generated; first token after {:.0} ms",
        report.prompt_tokens,
        report.generated.len(),
        report.ttft.as_secs_f64() * 1e3
    );
    if let Some(tpot) = report.tpot {
        eprintln!(
            "omnarc: {:.1} ms per output token ({:.2} tokens/s)",
            tpot.as_secs_f64() * 1e3,
            1.0 / tpot.as_secs_f64()
        );
    }
    Ok(())
}

fn platform() -> Platform {
    if cfg!(target_os = "android") {
        Platform::Android
    } else if cfg!(target_os = "ios") {
        Platform::Ios
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::Macos
    } else {
        Platform::Unspecified
    }
}

fn describe() -> DeviceInfo {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    let cpu_model = sys.cpus().first().map(|c| c.brand().trim().to_owned()).unwrap_or_default();
    DeviceInfo {
        name: System::host_name().unwrap_or_default(),
        platform: platform() as i32,
        os_version: System::long_os_version().unwrap_or_default(),
        cpu_model,
        cpu_cores: u32::try_from(sys.cpus().len()).unwrap_or(u32::MAX),
        memory_total_bytes: sys.total_memory(),
        agent_version: env!("CARGO_PKG_VERSION").to_owned(),
        ..DeviceInfo::default()
    }
}

#[allow(clippy::cast_precision_loss)] // display only
fn gib(bytes: u64) -> f64 {
    bytes as f64 / f64::from(1u32 << 30)
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Id { key } => match ring::node_id(&key.unwrap_or_else(ring::default_key_path)) {
            Ok(id) => println!("{id}"),
            Err(e) => {
                eprintln!("omnarc-node: {e}");
                std::process::exit(1);
            }
        },
        Command::Ring(args) => {
            if let Err(e) = ring(*args) {
                eprintln!("omnarc-node: {e}");
                std::process::exit(1);
            }
        }
        Command::Info { json } => {
            let info = describe();
            let platform = Platform::try_from(info.platform).unwrap_or(Platform::Unspecified);
            if json {
                let value = serde_json::json!({
                    "name": info.name,
                    "platform": platform.as_str_name(),
                    "os_version": info.os_version,
                    "cpu_model": info.cpu_model,
                    "cpu_cores": info.cpu_cores,
                    "memory_total_bytes": info.memory_total_bytes,
                    "agent_version": info.agent_version,
                });
                println!("{value:#}");
            } else {
                println!("Device       {}", info.name);
                println!("Platform     {}", platform.as_str_name());
                println!("OS           {}", info.os_version);
                println!("CPU          {} ({} cores)", info.cpu_model, info.cpu_cores);
                println!("Memory       {:.1} GiB", gib(info.memory_total_bytes));
                println!("Agent        {}", info.agent_version);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_this_device() {
        let info = describe();
        assert!(info.cpu_cores >= 1);
        assert!(info.memory_total_bytes > 0);
        assert_ne!(info.platform, Platform::Unspecified as i32);
        assert_eq!(info.agent_version, env!("CARGO_PKG_VERSION"));
    }
}
