//! Data returned by Terminus plus the pure helpers built around it.

use serde::{Deserialize, Deserializer, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Environments every Pantheon site has; they can't be deleted.
pub const PROTECTED_ENVS: [&str; 3] = ["dev", "test", "live"];

/// Names Pantheon refuses for multidev environments.
const RESERVED_MULTIDEV_NAMES: [&str; 12] = [
    "dev", "test", "live", "master", "settings", "team", "support", "debug", "multidev", "files",
    "tags", "billing",
];

const MAX_MULTIDEV_NAME_LEN: usize = 11;

// ── terminus data types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub name: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub plan_name: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub framework: Option<String>,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub frozen: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub domain: Option<String>,
    /// `"git"` or `"sftp"`.
    #[serde(default, deserialize_with = "lenient_string")]
    pub connection_mode: Option<String>,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub locked: Option<bool>,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub initialized: Option<bool>,
}

impl Environment {
    pub fn is_protected(&self) -> bool {
        PROTECTED_ENVS.contains(&self.id.as_str())
    }

    pub fn is_sftp(&self) -> bool {
        self.connection_mode.as_deref() == Some("sftp")
    }

    /// Code only flows dev → test → live, so only test and live accept deploys.
    pub fn deploy_source(&self) -> Option<&'static str> {
        match self.id.as_str() {
            "test" => Some("dev"),
            "live" => Some("test"),
            _ => None,
        }
    }

    pub fn url(&self) -> Option<String> {
        self.domain
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(|d| format!("https://{d}"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backup {
    pub file: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub size: Option<String>,
    #[serde(default, deserialize_with = "lenient_f64")]
    pub date: Option<f64>,
    #[serde(default, deserialize_with = "lenient_f64")]
    pub expiry: Option<f64>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub initiator: Option<String>,
}

impl Backup {
    /// `code`, `database` or `files`, taken from the archive name
    /// (`<site>_<env>_<timestamp>_<element>.tar.gz`).
    pub fn element(&self) -> &str {
        let stem = self.file.split('.').next().unwrap_or("");
        stem.rsplit('_')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("—")
    }
}

// ── ordering ──────────────────────────────────────────────────────────────────

/// Terminus returns sites keyed by UUID, so the raw order is meaningless.
pub fn sort_sites(sites: &mut [Site]) {
    sites.sort_by_key(|s| s.name.to_lowercase());
}

/// dev, test, live first (in pipeline order), then multidevs alphabetically.
pub fn sort_envs(envs: &mut [Environment]) {
    envs.sort_by(|a, b| {
        let rank = |e: &Environment| {
            PROTECTED_ENVS
                .iter()
                .position(|p| *p == e.id)
                .unwrap_or(PROTECTED_ENVS.len())
        };
        rank(a).cmp(&rank(b)).then_with(|| a.id.cmp(&b.id))
    });
}

/// Newest first.
pub fn sort_backups(backups: &mut [Backup]) {
    backups.sort_by(|a, b| {
        b.date
            .unwrap_or(0.0)
            .total_cmp(&a.date.unwrap_or(0.0))
            .then_with(|| a.file.cmp(&b.file))
    });
}

// ── validation & formatting ───────────────────────────────────────────────────

pub fn validate_multidev_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Multidev name is required.".into());
    }
    if name.len() > MAX_MULTIDEV_NAME_LEN {
        return Err(format!(
            "Multidev names are limited to {MAX_MULTIDEV_NAME_LEN} characters."
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("Use only lowercase letters, numbers and hyphens.".into());
    }
    // A leading hyphen would also be parsed as a CLI option by terminus.
    if name.starts_with('-') {
        return Err("Multidev names can't start with a hyphen.".into());
    }
    if RESERVED_MULTIDEV_NAMES.contains(&name) {
        return Err(format!("'{name}' is a reserved name."));
    }
    Ok(())
}

/// Case-insensitive substring match used by the `/` filters.
pub fn matches_filter(name: &str, filter: &str) -> bool {
    filter.is_empty() || name.to_lowercase().contains(&filter.to_lowercase())
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Human age of data fetched at `then` (Unix seconds): "just now", "5m ago"…
pub fn format_age(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then);
    match secs {
        0..10 => "just now".into(),
        10..60 => format!("{secs}s ago"),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

/// Formats a Unix timestamp as `YYYY-MM-DD HH:MM UTC`.
pub fn format_timestamp(secs: f64) -> String {
    let secs = secs.floor() as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60
    )
}

// Howard Hinnant's days → civil date algorithm (proleptic Gregorian calendar).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

// ── lenient deserializers ─────────────────────────────────────────────────────
// Terminus is not strict about JSON types (numbers as strings, bools as
// "true"/"1"...). One odd field must not make a whole list fail to load.

fn lenient_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s),
        other => Some(other.to_string()),
    })
}

fn lenient_bool<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Bool(b) => Some(b),
        serde_json::Value::Number(n) => n.as_i64().map(|n| n != 0),
        serde_json::Value::String(s) => match s.to_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" | "" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

fn lenient_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(name: &str) -> Site {
        Site {
            name: name.into(),
            plan_name: None,
            framework: None,
            frozen: None,
        }
    }

    fn env(id: &str) -> Environment {
        Environment {
            id: id.into(),
            domain: None,
            connection_mode: None,
            locked: None,
            initialized: None,
        }
    }

    #[test]
    fn sites_are_sorted_by_name_case_insensitively() {
        let mut sites = vec![site("beta"), site("Alpha"), site("gamma")];
        sort_sites(&mut sites);
        let names: Vec<_> = sites.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "beta", "gamma"]);
    }

    #[test]
    fn envs_follow_pipeline_order_then_multidevs() {
        let mut envs = vec![
            env("zeta"),
            env("live"),
            env("alpha"),
            env("dev"),
            env("test"),
        ];
        sort_envs(&mut envs);
        let ids: Vec<_> = envs.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["dev", "test", "live", "alpha", "zeta"]);
    }

    #[test]
    fn backups_are_newest_first() {
        let b = |file: &str, date: f64| Backup {
            file: file.into(),
            size: None,
            date: Some(date),
            expiry: None,
            initiator: None,
        };
        let mut backups = vec![b("old", 1.0), b("new", 3.0), b("mid", 2.0)];
        sort_backups(&mut backups);
        let files: Vec<_> = backups.iter().map(|b| b.file.as_str()).collect();
        assert_eq!(files, ["new", "mid", "old"]);
    }

    #[test]
    fn multidev_name_validation() {
        assert!(validate_multidev_name("feature-1").is_ok());
        assert!(validate_multidev_name("abcdefghijk").is_ok());
        assert!(validate_multidev_name("").is_err());
        assert!(validate_multidev_name("abcdefghijkl").is_err());
        assert!(validate_multidev_name("Feature").is_err());
        assert!(validate_multidev_name("my_branch").is_err());
        assert!(validate_multidev_name("-oops").is_err());
        assert!(validate_multidev_name("master").is_err());
        assert!(validate_multidev_name("live").is_err());
    }

    #[test]
    fn filter_is_case_insensitive_and_empty_matches_all() {
        assert!(matches_filter("My-Site", "my-s"));
        assert!(matches_filter("anything", ""));
        assert!(!matches_filter("site", "other"));
    }

    #[test]
    fn timestamps_are_formatted_in_utc() {
        assert_eq!(format_timestamp(0.0), "1970-01-01 00:00 UTC");
        assert_eq!(format_timestamp(1_707_971_195.539), "2024-02-15 04:26 UTC");
        assert_eq!(format_timestamp(951_782_400.0), "2000-02-29 00:00 UTC");
    }

    #[test]
    fn ages_are_human_readable() {
        assert_eq!(format_age(100, 105), "just now");
        assert_eq!(format_age(100, 145), "45s ago");
        assert_eq!(format_age(0, 3 * 60 + 5), "3m ago");
        assert_eq!(format_age(0, 2 * 3600), "2h ago");
        assert_eq!(format_age(0, 3 * 86_400), "3d ago");
        // Clock skew never yields a negative age.
        assert_eq!(format_age(200, 100), "just now");
    }

    #[test]
    fn backup_element_comes_from_file_name() {
        let b = |file: &str| Backup {
            file: file.into(),
            size: None,
            date: None,
            expiry: None,
            initiator: None,
        };
        assert_eq!(
            b("my_site_dev_2024-02-15T04-25-44_UTC_database.sql.gz").element(),
            "database"
        );
        assert_eq!(b("site_dev_2024_code.tar.gz").element(), "code");
        assert_eq!(b("").element(), "—");
    }

    #[test]
    fn environment_helpers() {
        let mut e = env("test");
        assert!(e.is_protected());
        assert_eq!(e.deploy_source(), Some("dev"));
        assert_eq!(env("live").deploy_source(), Some("test"));
        assert_eq!(env("dev").deploy_source(), None);
        assert_eq!(e.url(), None);
        e.domain = Some("test-x.pantheonsite.io".into());
        assert_eq!(e.url().as_deref(), Some("https://test-x.pantheonsite.io"));
        e.connection_mode = Some("sftp".into());
        assert!(e.is_sftp());
    }

    #[test]
    fn lenient_fields_accept_odd_types() {
        let e: Environment =
            serde_json::from_str(r#"{"id":"dev","locked":"false","initialized":1,"domain":null}"#)
                .unwrap();
        assert_eq!(e.locked, Some(false));
        assert_eq!(e.initialized, Some(true));
        assert_eq!(e.domain, None);

        let b: Backup =
            serde_json::from_str(r#"{"file":"f","size":12,"date":"1707971195.5"}"#).unwrap();
        assert_eq!(b.size.as_deref(), Some("12"));
        assert_eq!(b.date, Some(1_707_971_195.5));
    }
}
