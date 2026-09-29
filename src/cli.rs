use clap::{Parser, Subcommand};

/// Estimate Soroban contract resource costs with network config-drift tracking.
///
/// Wraps Stellar's `simulateTransaction` RPC and adds awareness of how the
/// network's resource-pricing configuration changes over time.
#[derive(Parser, Debug)]
#[command(name = "soroban-cost-estimator")]
#[command(about = "Estimate Soroban contract costs & track network pricing changes", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Simulate a single contract invocation and print the cost report.
    Estimate {
        /// Path to the compiled Soroban contract `.wasm` file.
        #[arg(long, short)]
        wasm: String,

        /// Network to simulate against.
        #[arg(long, default_value = "testnet")]
        network: String,

        /// Explicit RPC URL (overrides network-based resolution).
        #[arg(long)]
        rpc_url: Option<String>,

        /// Contract function name to invoke.
        #[arg(long)]
        r#fn: Option<String>,

        /// Deployed contract ID (64 hex chars) to invoke. Required when --fn is used.
        #[arg(long)]
        id: Option<String>,

        /// Function arguments as key=value pairs (value is type-inferred).
        #[arg(long = "arg", value_name = "KEY=VAL")]
        args: Vec<String>,

        /// Skip re-simulation when a cached estimate is still fresh
        /// (e.g. "30m", "1h", "7d"; bare value = seconds).
        #[arg(long, value_name = "DURATION")]
        cache_ttl: Option<String>,

        /// Output as JSON instead of a human-readable table.
        #[arg(long)]
        json: bool,
    },

    /// Enumerate all public contract functions and estimate each one.
    EstimateAll {
        /// Path to the compiled Soroban contract `.wasm` file.
        #[arg(long, short)]
        wasm: String,

        /// Network to simulate against.
        #[arg(long, default_value = "testnet")]
        network: String,

        /// Deployed contract ID (64 hex chars) to invoke each function against.
        #[arg(long)]
        id: Option<String>,

        /// Output as JSON instead of a human-readable list.
        #[arg(long)]
        json: bool,
    },

    /// Fetch and store a snapshot of the network's resource-pricing configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Poll network config on an interval and print diffs when they appear.
    Watch {
        /// Network to watch.
        #[arg(long, default_value = "testnet")]
        network: String,

        /// Polling interval (e.g. "30m", "1h").
        #[arg(long, default_value = "1h")]
        interval: String,
    },

    /// Inspect and manage the local estimate cache.
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum CacheAction {
    /// Check that every cached estimate is valid JSON and not corrupted.
    Verify {
        /// Delete corrupted cache entries instead of only reporting them.
        #[arg(long, visible_alias = "fix")]
        repair: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SnapshotAction {
    /// Display the complete configuration stored in a historical snapshot.
    ///
    /// Selects the snapshot to show by explicit file path, by timestamp
    /// (`--at`, exact or prefix within the network's snapshot history), or
    /// the most recent one (`--latest`, also the default with no selector).
    Show {
        /// Snapshot file path to display (overrides `--at` and `--latest`).
        snapshot: Option<String>,

        /// Timestamp (exact or prefix, e.g. `2026-08-04` or `2026-08-04T07`)
        /// of the snapshot to show. When several snapshots match the prefix,
        /// the most recent one is used.
        #[arg(long, conflicts_with = "snapshot")]
        at: Option<String>,

        /// Show the most recent snapshot for the network.
        #[arg(long, conflicts_with_all = ["snapshot", "at"])]
        latest: bool,

        /// Network whose snapshot history to search when no explicit path
        /// is given.
        #[arg(long)]
        network: Option<String>,

        /// Print the snapshot as JSON instead of a formatted report.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Fetch all ConfigSetting entries and save a timestamped snapshot.
    ///
    /// With no subcommand, fetches and saves a new snapshot. With the `show`
    /// subcommand, displays a stored one instead.
    Snapshot {
        #[command(subcommand)]
        action: Option<SnapshotAction>,

        /// Network to fetch config from.
        #[arg(long, default_value = "testnet")]
        network: String,

        /// Explicit output path (defaults to ~/.soroban-cost-estimator/snapshots/).
        #[arg(long)]
        out: Option<String>,

        /// Print the snapshot as JSON instead of the summary lines.
        #[arg(long)]
        json: bool,
    },

    /// Diff the current network config against the most recent snapshot.
    Diff {
        /// Network to compare against.
        #[arg(long, default_value = "testnet")]
        network: String,

        /// Explicit snapshot path to compare against (defaults to latest).
        #[arg(long)]
        against: Option<String>,
    },

    /// Show the full chronological change log across all stored snapshots.
    History {
        /// Network whose snapshot history to inspect.
        #[arg(long, default_value = "testnet")]
        network: String,
    },

    /// Show when each config setting last changed.
    LastChanged {
        /// Network whose snapshot history to inspect.
        #[arg(long, default_value = "testnet")]
        network: String,
    },
}
