//! Running `omnarc-node serve` as the device's own long-running service (SPEC NO-04): a systemd user unit on Linux, a
//! launch agent on macOS, a task at sign-in on Windows. The desktop app takes this over in stage B5.
//!
//! An installation is a plan — the files to write and the commands to run — so it can be shown before it is done.

use std::path::{Path, PathBuf};

use crate::ring::BoxError;

/// The service's name: the systemd unit, the launch agent's label, the Windows task.
pub const NAME: &str = "omnarc-node";

/// What installing or removing the service does.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// Files to write, with their text.
    pub write: Vec<(PathBuf, String)>,
    /// Files to delete.
    pub delete: Vec<PathBuf>,
    /// Commands to run, in order.
    pub run: Vec<Vec<String>>,
    /// What to tell the owner afterwards.
    pub note: String,
}

impl Plan {
    /// Writes the files and runs the commands.
    ///
    /// # Errors
    /// A file cannot be written or a command fails.
    pub fn apply(&self) -> Result<(), BoxError> {
        for (path, text) in &self.write {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, text)?;
        }
        // the commands run while the files are there: a unit is disabled before it is deleted
        for command in &self.run {
            let (program, args) = command.split_first().ok_or("an empty command")?;
            let status = std::process::Command::new(program).args(args).status()?;
            if !status.success() {
                return Err(format!("{} failed ({status})", command.join(" ")).into());
            }
        }
        for path in &self.delete {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    /// The plan as text, to show before it is done.
    #[must_use]
    pub fn describe(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for (path, text) in &self.write {
            let _ = writeln!(out, "# {}\n{text}", path.display());
        }
        for command in &self.run {
            let _ = writeln!(out, "{}", command.join(" "));
        }
        for path in &self.delete {
            let _ = writeln!(out, "rm {}", path.display());
        }
        out
    }
}

fn home() -> Result<PathBuf, BoxError> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| "no home directory".into())
}

/// A word of a command line, quoted for the shell when it needs it.
fn quoted(word: &str) -> String {
    if word.chars().all(|c| c.is_ascii_alphanumeric() || "/._-:=".contains(c)) {
        word.to_owned()
    } else {
        format!("\"{}\"", word.replace('"', "\\\""))
    }
}

/// The systemd user unit that runs `exe` with `args`.
#[must_use]
pub fn systemd_unit(exe: &Path, args: &[String]) -> String {
    let command: Vec<String> = std::iter::once(exe.display().to_string()).chain(args.iter().cloned()).collect();
    format!(
        "[Unit]\nDescription=Omnarc: this device in its cluster\nAfter=network-online.target\nWants=network-online.target\n\n\
         [Service]\nExecStart={}\nRestart=on-failure\nRestartSec=3\n\n[Install]\nWantedBy=default.target\n",
        command.iter().map(|w| quoted(w)).collect::<Vec<_>>().join(" ")
    )
}

/// The launch agent that runs `exe` with `args` at sign-in and keeps it running.
#[must_use]
pub fn launch_agent(exe: &Path, args: &[String], log: &Path) -> String {
    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let words = std::iter::once(exe.display().to_string())
        .chain(args.iter().cloned())
        .fold(String::new(), |out, w| out + "    <string>" + &escape(&w) + "</string>\n");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{NAME}</string>\n  <key>ProgramArguments</key>\n  <array>\n{words}  </array>\n\
         \x20 <key>RunAtLoad</key>\n  <true/>\n  <key>KeepAlive</key>\n  <true/>\n  <key>StandardOutPath</key>\n  <string>{log}</string>\n\
         \x20 <key>StandardErrorPath</key>\n  <string>{log}</string>\n</dict>\n</plist>\n",
        log = escape(&log.display().to_string())
    )
}

/// The plan that installs the service running `exe` with `args` on this system.
///
/// # Errors
/// The home directory is unknown, or the system has no service manager Omnarc knows.
pub fn install(exe: &Path, args: &[String]) -> Result<Plan, BoxError> {
    if cfg!(target_os = "linux") {
        let unit = home()?.join(".config/systemd/user").join(format!("{NAME}.service"));
        Ok(Plan {
            write: vec![(unit, systemd_unit(exe, args))],
            run: vec![
                vec!["systemctl".into(), "--user".into(), "daemon-reload".into()],
                vec!["systemctl".into(), "--user".into(), "enable".into(), "--now".into(), format!("{NAME}.service")],
            ],
            note: "It starts when you sign in. To keep it running when nobody is signed in: loginctl enable-linger"
                .into(),
            ..Plan::default()
        })
    } else if cfg!(target_os = "macos") {
        let home = home()?;
        let agent = home.join("Library/LaunchAgents").join(format!("{NAME}.plist"));
        let log = home.join("Library/Logs/omnarc-node.log");
        Ok(Plan {
            write: vec![(agent.clone(), launch_agent(exe, args, &log))],
            run: vec![vec!["launchctl".into(), "load".into(), "-w".into(), agent.display().to_string()]],
            note: format!("It starts when you sign in; its log: {}", log.display()),
            ..Plan::default()
        })
    } else if cfg!(windows) {
        let command: Vec<String> = std::iter::once(exe.display().to_string()).chain(args.iter().cloned()).collect();
        let line = command.iter().map(|w| quoted(w)).collect::<Vec<_>>().join(" ");
        Ok(Plan {
            run: vec![
                vec![
                    "schtasks".into(),
                    "/Create".into(),
                    "/TN".into(),
                    NAME.into(),
                    "/TR".into(),
                    line,
                    "/SC".into(),
                    "ONLOGON".into(),
                    "/RL".into(),
                    "LIMITED".into(),
                    "/F".into(),
                ],
                vec!["schtasks".into(), "/Run".into(), "/TN".into(), NAME.into()],
            ],
            note: "It starts when you sign in.".into(),
            ..Plan::default()
        })
    } else {
        Err("this system has no service manager Omnarc knows yet".into())
    }
}

/// The plan that stops and removes the service.
///
/// # Errors
/// The home directory is unknown, or the system has no service manager Omnarc knows.
pub fn uninstall() -> Result<Plan, BoxError> {
    if cfg!(target_os = "linux") {
        Ok(Plan {
            run: vec![
                vec!["systemctl".into(), "--user".into(), "disable".into(), "--now".into(), format!("{NAME}.service")],
                vec!["systemctl".into(), "--user".into(), "daemon-reload".into()],
            ],
            delete: vec![home()?.join(".config/systemd/user").join(format!("{NAME}.service"))],
            ..Plan::default()
        })
    } else if cfg!(target_os = "macos") {
        let agent = home()?.join("Library/LaunchAgents").join(format!("{NAME}.plist"));
        Ok(Plan {
            run: vec![vec!["launchctl".into(), "unload".into(), "-w".into(), agent.display().to_string()]],
            delete: vec![agent],
            ..Plan::default()
        })
    } else if cfg!(windows) {
        Ok(Plan {
            run: vec![vec!["schtasks".into(), "/Delete".into(), "/TN".into(), NAME.into(), "/F".into()]],
            ..Plan::default()
        })
    } else {
        Err("this system has no service manager Omnarc knows yet".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> Vec<String> {
        ["--data", "/home/a b/omnarc", "serve", "--panel", "/opt/omnarc/panel"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    #[test]
    fn a_systemd_unit_runs_serve_and_restarts_it() {
        let unit = systemd_unit(Path::new("/usr/local/bin/omnarc-node"), &args());
        assert!(unit.contains(
            "ExecStart=/usr/local/bin/omnarc-node --data \"/home/a b/omnarc\" serve --panel /opt/omnarc/panel\n"
        ));
        assert!(unit.contains("Restart=on-failure") && unit.contains("WantedBy=default.target"));
    }

    #[test]
    fn a_launch_agent_keeps_serve_alive() {
        let agent =
            launch_agent(Path::new("/Applications/omnarc-node"), &args(), Path::new("/Users/a/Library/Logs/x.log"));
        assert!(agent.contains("<string>/home/a b/omnarc</string>"));
        assert!(agent.contains("<key>KeepAlive</key>\n  <true/>"));
        assert!(agent.contains(&format!("<string>{NAME}</string>")));
    }

    #[test]
    fn this_system_has_a_plan_both_ways() {
        if let Ok(plan) = install(Path::new("/x/omnarc-node"), &args()) {
            assert_ne!(plan.run, Vec::<Vec<String>>::new());
            assert!(plan.describe().contains("serve"));
            assert_ne!(uninstall().unwrap().run, Vec::<Vec<String>>::new());
        }
    }
}
