use std::path::{Path, PathBuf};

use tracing::{debug, trace};

use crate::config_snapshot::model::ConfigSnapshot;
use crate::error::{AppError, AppResult};

/// Returns the base data directory: `~/.soroban-cost-estimator`.
fn data_dir() -> AppResult<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::General("could not determine home directory".to_string()))?;
    Ok(home.join(".soroban-cost-estimator"))
}

/// Returns the snapshots directory, creating it if needed.
fn snapshots_dir() -> AppResult<PathBuf> {
    let dir = data_dir()?.join("snapshots");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Returns the cache directory, creating it if needed.
pub fn cache_dir() -> AppResult<PathBuf> {
    let dir = data_dir()?.join("cache");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Saves a config snapshot to disk as a JSON file.
///
/// The filename is `{network}-{timestamp}.json` within the snapshots directory,
/// unless an explicit `--out` path is provided.
///
/// # Network calls
/// None — pure file I/O.
pub fn save_snapshot(snapshot: &ConfigSnapshot, out_path: Option<&str>) -> AppResult<PathBuf> {
    let path = match out_path {
        Some(p) => PathBuf::from(p),
        None => {
            let dir = snapshots_dir()?;
            let filename = format!(
                "{}-{}.json",
                snapshot.network,
                snapshot.timestamp.replace(':', "-")
            );
            dir.join(filename)
        }
    };

    let json = serde_json::to_string_pretty(snapshot)?;
    std::fs::write(&path, json)?;
    debug!(path = %path.display(), network = snapshot.network, ledger = snapshot.ledger, "snapshot saved");
    Ok(path)
}

/// Loads the most recent snapshot for a given network.
///
/// Scans the snapshots directory for files matching `{network}-*.json`
/// and returns the one with the latest timestamp in its filename.
///
/// # Network calls
/// None — pure file I/O.
pub fn load_latest_snapshot(network: &str) -> AppResult<ConfigSnapshot> {
    debug!(network, "loading latest snapshot");
    let dir = snapshots_dir()?;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(&format!("{}-", network)) && n.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    let latest = entries
        .into_iter()
        .last()
        .ok_or_else(|| AppError::NoSnapshots(network.to_string()))?;

    let content = std::fs::read_to_string(latest.path())?;
    let snapshot: ConfigSnapshot =
        serde_json::from_str(&content).map_err(|e| AppError::SnapshotParse(e.to_string()))?;
    trace!(network, ledger = snapshot.ledger, "latest snapshot loaded");
    Ok(snapshot)
}

/// Loads a specific snapshot from an explicit path.
///
/// # Network calls
/// None — pure file I/O.
pub fn load_snapshot_from_path(path: &str) -> AppResult<ConfigSnapshot> {
    debug!(path, "loading snapshot from path");
    let content = std::fs::read_to_string(path)?;
    let snapshot: ConfigSnapshot =
        serde_json::from_str(&content).map_err(|e| AppError::SnapshotParse(e.to_string()))?;
    trace!(
        network = snapshot.network,
        ledger = snapshot.ledger,
        "snapshot loaded from path"
    );
    Ok(snapshot)
}

/// Lists all available snapshots for a given network.
///
/// # Network calls
/// None — pure file I/O.
pub fn list_snapshots(network: &str) -> AppResult<Vec<PathBuf>> {
    let dir = snapshots_dir()?;
    let mut snapshots = Vec::new();

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&format!("{}-", network)) && name_str.ends_with(".json") {
            snapshots.push(entry.path());
        }
    }

    snapshots.sort();
    Ok(snapshots)
}

/// Resolves a snapshot by timestamp (exact or prefix) for a network.
///
/// Snapshots are stored as `{network}-{timestamp}.json` (colons replaced
/// with dashes), so matching is done over the timestamp-shaped part of the
/// filename: the stored timestamp is normalized to the same on-disk form
/// before comparison. A prefix like `2026-08-04` or `2026-08-04T07` selects
/// among all snapshots from that period; when several match, the **most
/// recent** is returned.
///
/// Returns [`AppError::SnapshotNotFound`] naming the requested timestamp
/// when nothing matches.
///
/// # Network calls
/// None — pure file I/O.
pub fn resolve_snapshot_by_timestamp(network: &str, timestamp: &str) -> AppResult<ConfigSnapshot> {
    let needle = on_disk_timestamp(timestamp);
    let mut best: Option<(String, PathBuf)> = None;

    for path in list_snapshots(network)? {
        let Some(stored) = snapshot_timestamp_part(&path, network) else {
            continue;
        };
        if !stored.starts_with(&needle) {
            continue;
        }
        // For a prefix match, keep the lexicographically greatest stored
        // timestamp — filenames sort chronologically by construction.
        if best.as_ref().is_none_or(|(b, _)| stored >= *b) {
            best = Some((stored, path));
        }
    }

    let (_, path) = best.ok_or_else(|| {
        AppError::SnapshotNotFound(format!(
            "no snapshot for network '{network}' at timestamp '{timestamp}'"
        ))
    })?;

    let content = std::fs::read_to_string(&path)?;
    serde_json::from_str(&content).map_err(|e| AppError::SnapshotParse(e.to_string()))
}

/// Counts the stored snapshots for a network.
///
/// # Network calls
/// None — pure file I/O.
pub fn count_snapshots(network: &str) -> AppResult<usize> {
    Ok(list_snapshots(network)?.len())
}

/// Normalizes a timestamp to its on-disk filename form: colons become
/// dashes, because `:` is not portable across filesystems.
fn on_disk_timestamp(timestamp: &str) -> String {
    timestamp.replace(':', "-")
}

/// Extracts the timestamp-shaped segment between the `{network}-` prefix and
/// the `.json` suffix of a snapshot filename. Returns `None` when the file
/// name does not follow the store's naming convention.
fn snapshot_timestamp_part(path: &Path, network: &str) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let prefix_len = network.len() + 1;
    if !name.starts_with(&format!("{network}-")) || !name.ends_with(".json") {
        return None;
    }
    let inner_len = name
        .len()
        .checked_sub(prefix_len)?
        .checked_sub(".json".len())?;
    if inner_len == 0 {
        return None;
    }
    Some(name.get(prefix_len..prefix_len + inner_len)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that redirect `HOME` (env vars are process-global).
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    /// Runs `f` with `HOME` pointed at a fresh temp dir, restoring it after.
    fn with_temp_home<F, R>(label: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = HOME_LOCK.lock().expect("test mutex");
        let tmp = std::env::temp_dir().join(format!("sce-store-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp home");
        let old_home = std::env::var_os("HOME");
        // SAFETY: serialized by HOME_LOCK; no other test in this binary
        // reads HOME.
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
        let result = f();
        // SAFETY: as above.
        if let Some(home) = old_home {
            unsafe {
                std::env::set_var("HOME", home);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
        result
    }

    fn test_snapshot(network: &str, timestamp: &str, ledger: u32) -> ConfigSnapshot {
        ConfigSnapshot {
            network: network.to_string(),
            timestamp: timestamp.to_string(),
            ledger,
            network_protocol_version: Some(22),
            contract_compute: None,
            contract_ledger_cost: None,
            contract_historical_data: None,
            contract_events: None,
            contract_bandwidth: None,
            state_archival: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn test_count_snapshots_counts_only_this_network() {
        with_temp_home("count", || {
            save_snapshot(
                &test_snapshot("testnet", "2026-08-01T00:00:00+00:00", 1),
                None,
            )
            .expect("save testnet");
            save_snapshot(
                &test_snapshot("mainnet", "2026-08-02T00:00:00+00:00", 2),
                None,
            )
            .expect("save mainnet");

            assert_eq!(count_snapshots("testnet").expect("count testnet"), 1);
            assert_eq!(count_snapshots("mainnet").expect("count mainnet"), 1);
            assert_eq!(count_snapshots("futurenet").expect("count futurenet"), 0);
        });
    }

    #[test]
    fn test_resolve_by_exact_timestamp() {
        with_temp_home("resolve-exact", || {
            save_snapshot(
                &test_snapshot("testnet", "2026-08-01T00:00:00+00:00", 1),
                None,
            )
            .expect("save 1");
            save_snapshot(
                &test_snapshot("testnet", "2026-08-02T00:00:00+00:00", 2),
                None,
            )
            .expect("save 2");

            let snap = resolve_snapshot_by_timestamp("testnet", "2026-08-02T00:00:00+00:00")
                .expect("exact timestamp resolves");
            assert_eq!(snap.ledger, 2);
        });
    }

    #[test]
    fn test_resolve_by_timestamp_prefix_picks_most_recent() {
        with_temp_home("resolve-prefix", || {
            save_snapshot(
                &test_snapshot("testnet", "2026-08-01T07:15:00+00:00", 1),
                None,
            )
            .expect("save 07:15");
            save_snapshot(
                &test_snapshot("testnet", "2026-08-01T19:45:00+00:00", 2),
                None,
            )
            .expect("save 19:45");
            save_snapshot(
                &test_snapshot("testnet", "2026-08-02T00:00:00+00:00", 3),
                None,
            )
            .expect("save next day");

            // Day prefix: two candidates from 2026-08-01, latest (19:45) wins.
            let day = resolve_snapshot_by_timestamp("testnet", "2026-08-01")
                .expect("day prefix resolves");
            assert_eq!(day.ledger, 2);

            // Hour prefix resolves within the day.
            let hour = resolve_snapshot_by_timestamp("testnet", "2026-08-01T07")
                .expect("hour prefix resolves");
            assert_eq!(hour.ledger, 1);
        });
    }

    #[test]
    fn test_resolve_by_unknown_timestamp_errors() {
        with_temp_home("resolve-missing", || {
            save_snapshot(
                &test_snapshot("testnet", "2026-08-01T00:00:00+00:00", 1),
                None,
            )
            .expect("save 1");

            let err = resolve_snapshot_by_timestamp("testnet", "2025-01-01")
                .expect_err("a timestamp with no snapshot must error");
            assert!(
                err.to_string().contains("Snapshot not found"),
                "error must be SnapshotNotFound; got: {err}"
            );
            assert!(
                err.to_string().contains("2025-01-01"),
                "error must name the requested timestamp; got: {err}"
            );
        });
    }
}
