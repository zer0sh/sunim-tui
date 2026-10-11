//! Interaction with the host OS outside of terminus.

use anyhow::{Result, bail};
use std::{io::Write, process::Stdio, time::Duration};
use tokio::process::Command;

/// Some openers (e.g. `xdg-open` launching a fresh browser) only return when
/// the browser exits; after this long we assume it worked and stop waiting.
const OPENER_GRACE: Duration = Duration::from_secs(5);

pub async fn open_url(url: &str) -> Result<String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if is_wsl() && which("wslview") {
        "wslview"
    } else {
        "xdg-open"
    };
    let mut child = Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("could not run `{opener}`: {e}"))?;

    // Still running after the grace period means the browser took over;
    // tokio reaps the child in the background.
    if let Ok(status) = tokio::time::timeout(OPENER_GRACE, child.wait()).await {
        let status = status?;
        if !status.success() {
            bail!("`{opener}` exited with {status}");
        }
    }
    Ok(format!("Opened {url}"))
}

fn is_wsl() -> bool {
    std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::fs::read_to_string("/proc/version")
            .map(|s| s.to_lowercase().contains("microsoft"))
            .unwrap_or(false)
}

fn which(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(cmd).is_file()))
}

/// Standard base64 encoding without third-party dependencies, used for OSC 52
/// terminal clipboard sequences.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0xf) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Copies `text` to system clipboard via OSC 52 terminal sequence and native commands.
pub fn copy_to_clipboard(text: &str) -> Result<String> {
    // 1. On macOS, try pbcopy
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    // 2. On Linux, try wl-copy or xclip
    #[cfg(not(target_os = "macos"))]
    {
        let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let cmd = if is_wayland { "wl-copy" } else { "xclip" };
        let args: &[&str] = if is_wayland {
            &[]
        } else {
            &["-selection", "clipboard"]
        };
        if let Ok(mut child) = std::process::Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    // 3. Always write OSC 52 sequence directly to stdout (works across modern terminals, SSH and tmux)
    let encoded = base64_encode(text.as_bytes());
    let osc52 = format!("\x1b]52;c;{encoded}\x07");
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(osc52.as_bytes());
    let _ = stdout.flush();

    Ok(format!("Copied '{text}' to clipboard"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(
            base64_encode(b"https://pantheon.io"),
            "aHR0cHM6Ly9wYW50aGVvbi5pbw=="
        );
    }
}
