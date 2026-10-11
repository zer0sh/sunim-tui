//! Persistent state on disk:
//! - settings in `~/.config/sunim-tui/config.toml` (token, selected sites);
//! - last Terminus results in `~/.cache/sunim-tui/cache.json`, shown
//!   instantly on the next launch while fresh data loads.

use crate::model::{Backup, Environment, Site};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

// ── config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Pantheon machine token, saved only after Terminus accepted it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Sites shown on the dashboard and in the sites list. `None` until the
    /// user completes the first-run site selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_sites: Option<Vec<String>>,
}

pub fn default_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("sunim-tui").join("config.toml"));
    }
    dirs::config_dir().map(|d| d.join("sunim-tui").join("config.toml"))
}

/// A missing file is not an error: it just means a fresh install.
pub fn load(path: &Path) -> Result<Config> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e).with_context(|| format!("could not read {}", path.display())),
    };
    let mut cfg: Config =
        toml::from_str(&content).with_context(|| format!("invalid config {}", path.display()))?;
    cfg.token = cfg.token.filter(|t| !t.trim().is_empty());
    Ok(cfg)
}

pub fn save(path: &Path, cfg: &Config) -> Result<()> {
    let content = toml::to_string(cfg).context("could not serialize config")?;
    write_private(path, content.as_bytes())
}

// ── cache ─────────────────────────────────────────────────────────────────────

/// Bump when the cache layout changes; older files are then ignored.
pub const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cached<T> {
    /// Unix seconds.
    pub fetched_at: u64,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub version: u32,
    /// Account the data belongs to; the cache is discarded for anyone else.
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub sites: Option<Cached<Site>>,
    /// Keyed by site name.
    #[serde(default)]
    pub envs: BTreeMap<String, Cached<Environment>>,
    /// Keyed by `site.env`.
    #[serde(default)]
    pub backups: BTreeMap<String, Cached<Backup>>,
}

pub fn default_cache_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("sunim-tui").join("cache.json"));
    }
    dirs::cache_dir().map(|d| d.join("sunim-tui").join("cache.json"))
}

/// The cache is disposable: anything unreadable or from another version is
/// simply treated as empty.
pub fn load_cache(path: &Path) -> Cache {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Cache>(&content).ok())
        .filter(|cache| cache.version == CACHE_VERSION)
        .unwrap_or_default()
}

pub fn save_cache(path: &Path, cache: &Cache) -> Result<()> {
    let cache = Cache {
        version: CACHE_VERSION,
        ..cache.clone()
    };
    let content = serde_json::to_vec(&cache).context("could not serialize cache")?;
    write_private(path, &content)
}

pub fn remove_cache(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("could not remove {}", path.display()))
        }
        _ => Ok(()),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Writes through a temp file + rename so a crash never leaves a half-written
/// file, and creates it as 0600 inside a 0700 directory so it is never
/// readable by other users, not even briefly.
fn write_private(path: &Path, content: &[u8]) -> Result<()> {
    let dir = path.parent().context("path has no parent directory")?;
    fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("could not restrict {}", dir.display()))?;

    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .with_context(|| format!("could not write {}", tmp.display()))?;
        // `mode` only applies on creation; fix a leftover temp file too.
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(content)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("could not replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A path named `file` inside a fresh, unique temp directory.
    pub fn temp_path(file: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "sunim-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        dir.join(file)
    }

    pub fn temp_config_path() -> PathBuf {
        temp_path("config.toml")
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn missing_file_loads_default() {
        let path = temp_config_path();
        assert_eq!(load(&path).unwrap(), Config::default());
    }

    #[test]
    fn save_then_load_roundtrips_with_private_permissions() {
        let path = temp_config_path();
        let cfg = Config {
            token: Some("secret".into()),
            selected_sites: Some(vec!["a".into(), "b".into()]),
        };
        save(&path, &cfg).unwrap();
        assert_eq!(load(&path).unwrap(), cfg);
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        assert!(!path.with_extension("toml.tmp").exists());
    }

    #[test]
    fn old_config_with_only_a_token_still_loads() {
        let path = temp_config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "token = \"abc\"\n").unwrap();
        let cfg = load(&path).unwrap();
        assert_eq!(cfg.token.as_deref(), Some("abc"));
        assert_eq!(cfg.selected_sites, None);
    }

    #[test]
    fn blank_token_is_ignored_and_garbage_is_an_error() {
        let path = temp_config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "token = \"  \"\n").unwrap();
        assert_eq!(load(&path).unwrap().token, None);

        fs::write(&path, "this is = = not toml").unwrap();
        assert!(load(&path).is_err());
    }

    fn sample_cache() -> Cache {
        let site = Site {
            name: "a".into(),
            plan_name: Some("Basic".into()),
            framework: None,
            frozen: Some(false),
        };
        let env = Environment {
            id: "dev".into(),
            domain: Some("dev-a.pantheonsite.io".into()),
            connection_mode: Some("git".into()),
            locked: Some(false),
            initialized: Some(true),
        };
        let backup = Backup {
            file: "a_dev_2024_code.tar.gz".into(),
            size: Some("1MB".into()),
            date: Some(1.5),
            expiry: None,
            initiator: Some("manual".into()),
        };
        Cache {
            version: CACHE_VERSION,
            user: Some("me@example.com".into()),
            sites: Some(Cached {
                fetched_at: 10,
                items: vec![site],
            }),
            envs: BTreeMap::from([(
                "a".into(),
                Cached {
                    fetched_at: 11,
                    items: vec![env],
                },
            )]),
            backups: BTreeMap::from([(
                "a.dev".into(),
                Cached {
                    fetched_at: 12,
                    items: vec![backup],
                },
            )]),
        }
    }

    #[test]
    fn cache_roundtrips_privately() {
        let path = temp_path("cache.json");
        let cache = sample_cache();
        save_cache(&path, &cache).unwrap();
        assert_eq!(load_cache(&path), cache);
        assert_eq!(mode(&path), 0o600);

        remove_cache(&path).unwrap();
        assert!(!path.exists());
        remove_cache(&path).unwrap(); // removing twice is fine
    }

    #[test]
    fn unreadable_or_outdated_cache_is_empty() {
        let path = temp_path("cache.json");
        assert_eq!(load_cache(&path), Cache::default());

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load_cache(&path), Cache::default());

        let mut old = sample_cache();
        old.version = CACHE_VERSION + 1;
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(load_cache(&path), Cache::default());
    }
}
