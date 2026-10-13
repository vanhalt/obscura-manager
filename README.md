# obscura-manager

Ratatui terminal manager for the [Obscura](https://github.com/h4ckf0r0day/obscura) headless browser. Spawn, monitor, and drive every Obscura CLI feature from one keyboard-first TUI.

Verified against `obscura 0.2.4` at `/home/vht/.local/bin/obscura` (with sibling `obscura-worker`).

## Install / Run

```bash
cargo build --release
OBSCURA_ROOT=~/.local/share/obscura-manager ./target/release/obscura-manager
# or: ./target/release/obscura-manager --root /path/to/root
```

Binary discovery: `OBSCURA_BIN` > `PATH` > `$OBSCURA_ROOT/bin/obscura`.

## OBSCURA_ROOT layout

```
$OBSCURA_ROOT/
  artifacts/{screenshots,dumps,scrapes,pdfs,exports,logs}/
  instances.json   # history only — no reattach
  config.json      # env overrides (mirror: config.toml)
```

Fetch/scrape outputs and CDP screenshots/PDFs land in `artifacts/` by default.

## Tabs & Keys

| Tab | Keys |
|---|---|
| Dashboard (1) | Overview, instance counts, quick start |
| Tasks (2) | `n/f` fetch, `v` serve, `p` scrape, `m` mcp, `j/k` nav, `s` stop, `r` restart-as-new, `d` delete, `/` filter |
| CDP (3) | `c` connect to running serve, `x` disconnect, `t` targets, `j/k` method, `e` edit JSON, `Enter` send |
| Artifacts (4) | `j/k` nav, `o` open, `d` delete, `e` path, `r` refresh |
| Config (5) | `j/k` nav, `Enter/e` edit `OBSCURA_*` var, `d` clear |
| Logs (6) | `j/k` select instance, `PgUp/PgDn` scroll |
| Help (7) | Full keymap (`?` from anywhere) |

Global: `q`/`Ctrl+C` quit, `1-7` jump tab, `Tab` cycle, `?` help. Form: `Tab` next field, `Enter/Space` cycle/edit, `1-4` switch kind, `F5`/`Shift+S` spawn, `Esc` cancel.

**No reattach:** once an instance is `Exited`/`Failed`/`Killed` it stays dead. Press `r` to spawn a fresh instance (new ID/PID) with the same parameters. Same after manager restart — history is inspection-only.

## Coverage

All live flags for `fetch` (incl. `--file/--concurrency`), `serve` (incl. `--max-connections/--ready-file/--font-dir`), `scrape`, `mcp`, all global flags, all documented `OBSCURA_*` env vars, and the CDP domains (Target/Page/Runtime/DOM/Network/Fetch/IO/Storage/Input/LP) plus MCP tool reference.

## Tests

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
