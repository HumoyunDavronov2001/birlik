//! `birlik-node` — the agent that runs on every device.
//!
//! Stage B0 ships the skeleton: `birlik-node info` describes this device in the same shape
//! (`birlik.v1.DeviceInfo`) that it will announce to a cluster.

use birlik_protocol::v1::{DeviceInfo, Platform};
use clap::{Parser, Subcommand};
use sysinfo::System;

#[derive(Debug, Parser)]
#[command(name = "birlik-node", version, about = "The BIRLIK agent that runs on every device")]
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
