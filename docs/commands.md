# Command Reference

This page provides an index of all commands available in the `soroban-cost-estimator`.

- [estimate](commands/estimate.md)
- [estimate-all](commands/estimate-all.md)
- [watch](commands/watch.md)
- [config snapshot](commands/config-snapshot.md)
- [config snapshot show](commands/config-snapshot.md#config-snapshot-show)
- [config diff](commands/config-diff.md)
- [cache verify](#cache-verify)

## cache verify

Check that every cached estimate in `~/.soroban-cost-estimator/cache/` is
still valid JSON, parses as a cache entry, and still matches its cache key
(WASM hash + function + args hash). See the
[`cache verify`](../README.md#cache-verify) section of the README for flags,
exit codes, and `--repair`.
