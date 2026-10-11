//! Async wrappers around the `terminus` CLI.
//!
//! Every call runs `terminus` as a child process with stdin closed,
//! `--no-interaction` and a timeout, so a hung or interactive command can never
//! freeze the UI. Terminus keeps its own session on disk, so no token is passed
//! around except to `auth:login`.

use anyhow::{Result, anyhow, bail};
use serde::de::DeserializeOwned;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::Command;

use crate::model::{self, Backup, Environment, Site};

/// Read-only queries (lists, whoami, login).
const QUERY_TIMEOUT: Duration = Duration::from_secs(120);
/// Commands that wait for a Pantheon workflow (deploys, multidevs, backups…).
const WORKFLOW_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub const DEPLOY_NOTE: &str = "Deployed from sunim";

struct Output {
    stdout: String,
    stderr: String,
}

impl Output {
    /// Terminus prints its notices on stderr and most commands print nothing on
    /// stdout, so use whatever it said, or `fallback` when it said nothing.
    fn message(&self, fallback: String) -> String {
        [&self.stdout, &self.stderr]
            .into_iter()
            .map(|s| clean_message(s))
            .find(|s| !s.is_empty())
            .unwrap_or(fallback)
    }
}

pub fn terminus_bin() -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("terminus");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    // Check standard macOS Homebrew / system locations
    for path in ["/opt/homebrew/bin/terminus", "/usr/local/bin/terminus"] {
        let candidate = Path::new(path);
        if candidate.is_file() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

pub fn is_installed() -> bool {
    terminus_bin().is_some()
}

async fn run(args: &[&str], timeout: Duration) -> Result<Output> {
    let bin = terminus_bin().ok_or_else(|| anyhow!("`terminus` was not found in $PATH"))?;
    let command = args.first().copied().unwrap_or("terminus");
    let child = Command::new(bin)
        .args(args)
        .args(["--no-interaction", "--no-ansi"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Dropping the future on timeout kills the process instead of leaking it.
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow!("`terminus` was not found in $PATH"),
            _ => anyhow!("could not start terminus: {e}"),
        })?;

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            anyhow!(
                "`terminus {command}` timed out after {} s",
                timeout.as_secs()
            )
        })??;

    let out = Output {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };
    if output.status.success() {
        Ok(out)
    } else {
        let msg = out.message(format!("`terminus {command}` failed ({})", output.status));
        Err(anyhow!(msg))
    }
}

/// Turns terminus' log output (`[error]  Something went wrong`) into one
/// readable line.
fn clean_message(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            let line = line.trim();
            match (line.starts_with('['), line.find(']')) {
                (true, Some(end)) => line[end + 1..].trim(),
                _ => line,
            }
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// Terminus (or PHP warnings) may print text before the JSON payload, and that
// text can itself contain brackets (`[notice] …`), so return the first
// position that actually parses as JSON.
fn extract_json(raw: &str) -> Result<serde_json::Value> {
    for (start, _) in raw.match_indices(['{', '[']) {
        let mut values =
            serde_json::Deserializer::from_str(&raw[start..]).into_iter::<serde_json::Value>();
        if let Some(Ok(value)) = values.next() {
            return Ok(value);
        }
    }
    bail!("No JSON in terminus output: {}", raw.trim())
}

// Terminus returns JSON as either an array `[...]` or an object map
// `{"key": {...}, ...}`. Both cases produce a flat Vec<T>; one malformed entry
// fails the whole list instead of silently disappearing. Empty output means an
// empty list.
fn parse_list<T: DeserializeOwned>(raw: &str) -> Result<Vec<T>> {
    if raw.trim().is_empty() {
        return Ok(vec![]);
    }
    match extract_json(raw)? {
        serde_json::Value::Array(items) => items
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                serde_json::from_value(v).map_err(|e| anyhow!("unexpected data at #{i}: {e}"))
            })
            .collect(),
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, v)| {
                serde_json::from_value(v).map_err(|e| anyhow!("unexpected data for '{key}': {e}"))
            })
            .collect(),
        other => bail!("expected a JSON list from terminus, got: {other}"),
    }
}

// ── auth ──────────────────────────────────────────────────────────────────────

pub async fn whoami() -> Result<String> {
    let out = run(&["auth:whoami"], QUERY_TIMEOUT).await?;
    let user = out.stdout.trim();
    if user.is_empty() {
        bail!("not logged in");
    }
    Ok(user.to_string())
}

pub async fn login(token: &str) -> Result<String> {
    run(
        &["auth:login", &format!("--machine-token={token}")],
        QUERY_TIMEOUT,
    )
    .await?;
    whoami().await
}

/// Reuses the terminus session if it is still valid; otherwise logs in again
/// with the stored token (if any).
pub async fn restore_session(token: Option<String>) -> Result<String> {
    match whoami().await {
        Ok(user) => Ok(user),
        Err(e) => match token {
            Some(token) => login(&token).await,
            None => Err(e),
        },
    }
}

pub async fn logout() -> Result<()> {
    run(&["auth:logout"], QUERY_TIMEOUT).await?;
    Ok(())
}

// ── queries ───────────────────────────────────────────────────────────────────

pub async fn site_list() -> Result<Vec<Site>> {
    let out = run(&["site:list", "--format=json"], QUERY_TIMEOUT).await?;
    let mut sites = parse_list(&out.stdout)?;
    model::sort_sites(&mut sites);
    Ok(sites)
}

pub async fn env_list(site: &str) -> Result<Vec<Environment>> {
    let out = run(&["env:list", site, "--format=json"], QUERY_TIMEOUT).await?;
    let mut envs = parse_list(&out.stdout)?;
    model::sort_envs(&mut envs);
    Ok(envs)
}

pub async fn backup_list(site: &str, env: &str) -> Result<Vec<Backup>> {
    let target = format!("{site}.{env}");
    let out = run(&["backup:list", &target, "--format=json"], QUERY_TIMEOUT).await?;
    let mut backups = parse_list(&out.stdout)?;
    model::sort_backups(&mut backups);
    Ok(backups)
}

// ── operations ────────────────────────────────────────────────────────────────

pub async fn clear_cache(site: &str, env: &str) -> Result<String> {
    let target = format!("{site}.{env}");
    let out = run(&["env:clear-cache", &target], WORKFLOW_TIMEOUT).await?;
    Ok(out.message(format!("Cache cleared on {target}")))
}

pub async fn create_multidev(site: &str, from_env: &str, name: &str) -> Result<String> {
    let from = format!("{site}.{from_env}");
    let out = run(&["multidev:create", &from, name], WORKFLOW_TIMEOUT).await?;
    Ok(out.message(format!("Multidev '{name}' created from '{from_env}'")))
}

/// The caller must have asked the user for confirmation already.
pub async fn delete_multidev(site: &str, env: &str, delete_branch: bool) -> Result<String> {
    let target = format!("{site}.{env}");
    let mut args = vec!["multidev:delete", target.as_str()];
    if delete_branch {
        args.push("--delete-branch");
    }
    args.push("--yes");
    let out = run(&args, WORKFLOW_TIMEOUT).await?;
    Ok(out.message(format!("Deleted {target}")))
}

/// The caller must have asked the user for confirmation already.
pub async fn deploy(site: &str, env: &str) -> Result<String> {
    let target = format!("{site}.{env}");
    let note = format!("--note={DEPLOY_NOTE}");
    let out = run(&["env:deploy", &target, &note, "--yes"], WORKFLOW_TIMEOUT).await?;
    Ok(out.message(format!("Deployed code to {target}")))
}

/// `mode` is `"git"` or `"sftp"`. Switching to git discards uncommitted SFTP
/// changes, so the caller must have asked for confirmation.
pub async fn set_connection_mode(site: &str, env: &str, mode: &str) -> Result<String> {
    let target = format!("{site}.{env}");
    let out = run(
        &["connection:set", &target, mode, "--yes"],
        WORKFLOW_TIMEOUT,
    )
    .await?;
    Ok(out.message(format!("{target} is now in {mode} mode")))
}

pub async fn wake(site: &str, env: &str) -> Result<String> {
    let target = format!("{site}.{env}");
    let out = run(&["env:wake", &target], QUERY_TIMEOUT).await?;
    Ok(out.message(format!("{target} is awake")))
}

pub async fn create_backup(site: &str, env: &str) -> Result<String> {
    let target = format!("{site}.{env}");
    let out = run(&["backup:create", &target], WORKFLOW_TIMEOUT).await?;
    Ok(out.message(format!("Backup created for {target}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_skips_leading_noise() {
        let v = extract_json("[notice] hi\n{\"a\":1}").unwrap();
        assert_eq!(v["a"], 1);
        let v = extract_json("Deprecated: foo() in bar.php\n[1,2]").unwrap();
        assert_eq!(v[1], 2);
        assert!(extract_json("no json here").is_err());
        assert!(extract_json("[notice] only a log line").is_err());
    }

    #[test]
    fn parse_list_accepts_arrays_maps_and_empty_output() {
        let from_array: Vec<Site> = parse_list(r#"[{"name":"b"},{"name":"a"}]"#).unwrap();
        assert_eq!(from_array.len(), 2);

        let from_map: Vec<Site> =
            parse_list(r#"{"uuid-1":{"name":"a","plan_name":"Basic"},"uuid-2":{"name":"b"}}"#)
                .unwrap();
        assert_eq!(from_map.len(), 2);
        assert!(
            from_map
                .iter()
                .any(|s| s.plan_name.as_deref() == Some("Basic"))
        );

        let empty: Vec<Site> = parse_list("  \n").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn parse_list_reports_bad_entries_instead_of_dropping_them() {
        let err = parse_list::<Site>(r#"{"uuid-1":{"name":"a"},"uuid-2":{"nope":1}}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("uuid-2"), "{err}");
    }

    #[test]
    fn parse_list_reads_real_terminus_env_output() {
        let raw = r#"{
            "dev": {"id":"dev","created":1697580123,"domain":"dev-x.pantheonsite.io",
                    "connection_mode":"git","locked":false,"initialized":true}
        }"#;
        let envs: Vec<Environment> = parse_list(raw).unwrap();
        assert_eq!(envs[0].connection_mode.as_deref(), Some("git"));
        assert_eq!(envs[0].initialized, Some(true));
    }

    #[test]
    fn clean_message_strips_log_tags() {
        assert_eq!(
            clean_message(" [error]  The site was not found. \n\n"),
            "The site was not found."
        );
        assert_eq!(
            clean_message("[notice] Deploying\n[notice] Done"),
            "Deploying Done"
        );
        assert_eq!(clean_message(""), "");
    }

    #[test]
    fn output_message_prefers_stdout_then_stderr_then_fallback() {
        let out = |stdout: &str, stderr: &str| Output {
            stdout: stdout.into(),
            stderr: stderr.into(),
        };
        assert_eq!(out("ok", "note").message("fb".into()), "ok");
        assert_eq!(out("", "[notice] note").message("fb".into()), "note");
        assert_eq!(out(" ", "").message("fb".into()), "fb");
    }
}
