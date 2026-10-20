//! Foreground activation is separate from selecting an agent's internal pane.
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub parent: u32,
    pub executable: String,
}
pub fn parse_processes(text: &str) -> Vec<Process> {
    text.lines()
        .filter_map(|line| {
            let (pid, rest) = line.trim().split_once(char::is_whitespace)?;
            let (parent, executable) = rest.trim().split_once(char::is_whitespace)?;
            Some(Process {
                pid: pid.parse().ok()?,
                parent: parent.parse().ok()?,
                executable: executable.trim().into(),
            })
        })
        .collect()
}
pub fn herdr_hosts(processes: &[Process]) -> Vec<String> {
    let table: HashMap<u32, &Process> = processes.iter().map(|p| (p.pid, p)).collect();
    let mut hosts = HashSet::new();
    for child in processes.iter().filter(|p| {
        std::path::Path::new(&p.executable)
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s == "herdr" || s == "herdr.exe")
    }) {
        let mut parent = child.parent;
        let mut visited = HashSet::new();
        while let Some(p) = table.get(&parent) {
            if !visited.insert(parent) {
                break;
            }
            if let Some(i) = p.executable.find(".app/") {
                hosts.insert(p.executable[..i + 4].to_string());
                break;
            }
            let name = std::path::Path::new(&p.executable)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if [
                "gnome-terminal-server",
                "kitty",
                "alacritty",
                "wezterm-gui",
                "konsole",
                "xterm",
                "xfce4-terminal",
                "foot",
            ]
            .contains(&name)
            {
                hosts.insert(p.pid.to_string());
                break;
            }
            parent = p.parent;
        }
    }
    let mut hosts: Vec<_> = hosts.into_iter().collect();
    hosts.sort();
    hosts
}
pub async fn herdr_terminal() -> Result<(), String> {
    #[cfg(unix)]
    {
        let processes = crate::agents::output("/bin/ps", &["-axo", "pid=,ppid=,comm="]).await?;
        let hosts = herdr_hosts(&parse_processes(&processes));
        let host=match hosts.as_slice(){[host]=>host,_=>return Err(format!("Pane selected, but found {} terminal hosts. Bring the Herdr terminal forward manually.",hosts.len()))};
        #[cfg(target_os = "macos")]
        crate::agents::output("/usr/bin/open", &["-a", host]).await?;
        #[cfg(not(target_os = "macos"))]
        {
            let ids = crate::agents::output("xdotool", &["search", "--pid", host]).await?;
            let id = ids
                .lines()
                .next()
                .ok_or("Pane selected, but terminal window was not found")?;
            crate::agents::output("xdotool", &["windowactivate", "--sync", id]).await?;
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        Err("Pane selected. Windows terminal activation is not configured; bring the terminal forward manually.".into())
    }
}
pub fn codex_url(target: &str) -> Result<String, String> {
    if target.is_empty()
        || !target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Missing or invalid Codex thread ID".into());
    }
    Ok(format!("codex://threads/{target}"))
}
pub async fn codex(session: &crate::model::AgentSession) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // The official desktop deep link targets the actual thread rather than an arbitrary window.
        let url = codex_url(&session.target)?;
        crate::agents::output("/usr/bin/open", &[&url]).await?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = session;
        Err("This Codex desktop focus adapter is unavailable on this platform".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deep_links_preserve_child_identity_and_reject_injection() {
        assert_eq!(codex_url("child-123").unwrap(), "codex://threads/child-123");
        for invalid in ["", "child/parent", "id?prompt=hello", "--bad;open", "id\n"] {
            assert!(codex_url(invalid).is_err());
        }
    }
    #[test]
    fn finds_terminal_ancestor_and_ignores_detached_server() {
        let p=parse_processes("101 1 /System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal\n102 101 /bin/zsh\n103 102 /opt/homebrew/bin/herdr\n200 1 /opt/homebrew/bin/herdr");
        assert_eq!(
            herdr_hosts(&p),
            ["/System/Applications/Utilities/Terminal.app"]
        );
    }
    #[test]
    fn rejects_ambiguity_without_assuming_focused_session() {
        let p = parse_processes(
            "1 0 /A.app/Contents/MacOS/A\n2 1 herdr\n3 0 /B.app/Contents/MacOS/B\n4 3 herdr",
        );
        assert_eq!(herdr_hosts(&p).len(), 2);
    }
    #[test]
    fn ancestry_cycles_and_malformed_ps_do_not_loop() {
        let p = parse_processes("bad line\n1 2 herdr\n2 1 /bin/zsh");
        assert!(herdr_hosts(&p).is_empty());
    }
}
