# shlog — Transparent CLI Command Logger for Agentic Systems

## Problem

When AI agents (Claude Code, etc.) execute CLI commands on your behalf, there's no visibility into what they actually ran. If something goes wrong — a bad `git push`, an unintended `rm`, a failing `terraform apply` — you're left digging through terminal scrollback or agent conversation history to reconstruct what happened.

## Solution

**shlog** is a transparent shell shim that logs every CLI command an agent executes, without the agent knowing it exists. Zero token overhead, zero behavioral change. It sits between the agent and your real shell, records what happened, and gets out of the way.

## Scope

### In scope (POC)

- Shell shim that intercepts commands via `SHELL` environment variable override
- Structured logging to JSONL
- CLI tool for querying, searching, and tailing logs
- Activation/deactivation via shell profile or per-session wrapping
- macOS only

### Out of scope (POC)

- Web dashboard (future)
- MCP calls, tool calls, web fetches — only CLI commands
- Session detection / per-session log splitting (research item)
- Log rotation
- Linux / Windows support
- PATH shim approach (future, if needed for direct exec coverage)

## Architecture

Single Rust binary with two modes, determined by invocation:

- **Shim mode**: invoked with `-c <command>` (as a shell replacement) — logs and delegates
- **CLI mode**: invoked with a subcommand (`log`, `tail`, `search`, etc.) — queries logs

### Components

| Component | Responsibility |
|-----------|---------------|
| `shim`    | Intercept command, fork, exec real shell, capture exit code, log entry |
| `store`   | Read/write JSONL log entries to `~/.shlog/logs/commands.jsonl` |
| `cli`     | User-facing query interface (subcommands) |
| `config`  | Minimal configuration (real shell path, log location) |

### Data Flow

```
Agent (e.g. Claude Code)
  → spawns $SHELL -c "git push origin main"
  → shlog shim intercepts
  → records start timestamp
  → forks child process
  → child: exec real shell (/bin/zsh -c "git push origin main")
  → parent: waits for child, captures exit code
  → computes duration
  → appends log entry to ~/.shlog/logs/commands.jsonl
  → exits with child's exit code
```

## Log Format

**Location:** `~/.shlog/logs/commands.jsonl`

**Format:** JSON Lines — one JSON object per line.

```json
{
  "ts": "2026-03-30T14:32:01.123Z",
  "cmd": "git push origin main",
  "cwd": "/Users/mathewbravo/dev/myproject",
  "exit_code": 0,
  "shell": "/bin/zsh",
  "ppid": 48291,
  "duration_ms": 1423
}
```

| Field         | Type   | Description |
|---------------|--------|-------------|
| `ts`          | string | ISO 8601 timestamp, UTC |
| `cmd`         | string | Full command string passed to `-c` |
| `cwd`         | string | Working directory at invocation |
| `exit_code`   | int    | Process exit code (128+N for signals) |
| `shell`       | string | Real shell that executed the command |
| `ppid`        | int    | Parent process ID |
| `duration_ms` | int    | Command execution time in milliseconds |

### Concurrent Write Safety

POSIX guarantees atomic appends under `PIPE_BUF` (4096 bytes on macOS). A typical log entry is 200-300 bytes — well under the limit. No locking needed for concurrent shlog instances writing to the same file.

## The Shim

### Invocation Detection

```
if args contain "-c" → shim mode (log and delegate)
if args contain subcommand → CLI mode
if no "-c" flag → exec real shell directly (interactive shell, no logging)
```

### Fork + Exec Strategy

The shim forks rather than directly exec-ing the real shell. This allows the parent process to wait for the child, capture the exit code and duration, and write the log entry after the command completes.

The child process `exec`s the real shell with the original arguments. The parent waits, logs, and exits with the child's exit code.

### Performance

- No async runtime — `std` Rust only on the shim path
- No config file parsing on the hot path — real shell path comes from `$SHLOG_REAL_SHELL` env var
- Log append is a single `write()` syscall, no fsync
- Total overhead: microseconds per command

### Real Shell Resolution

Priority order:
1. `$SHLOG_REAL_SHELL` environment variable (set by `shlog activate`)
2. `~/.shlog/config.toml`
3. Fallback: `/bin/zsh`

## Edge Cases

### Interactive Shell Launch

If `$SHELL` is invoked without `-c` (e.g., opening a new terminal tab), shlog detects the absence of `-c` and execs the real shell directly. No logging, no interference.

### Nested Shlog

If a command spawns a subshell and `SHELL=shlog`, you get shlog-inside-shlog. Solved with a guard: the outer shlog sets `SHLOG_PASSTHROUGH=1` in the child's environment. Inner shlog instances check this and exec the real shell immediately without logging. Only the outermost invocation logs.

### Signals

shlog forwards SIGINT, SIGTERM, and other signals to the child process. If the child is killed by a signal, shlog logs exit code as `128 + signal_number` (standard convention) and exits the same way.

### Logging Failure

If logging fails (disk full, permissions, missing directory), shlog silently drops the log entry. The real command still executes and the exit code is still propagated. Logging never blocks or fails the user's command.

### Corrupted Log Lines

The CLI skips malformed JSONL lines (e.g., from a partial write during a crash) and continues processing. No query fails because of a single bad line.

## CLI Interface

### Subcommands

```bash
# View recent commands (default: last 50)
shlog log
shlog log -n 100

# Live tail
shlog tail

# Search and filter
shlog search "git push"              # substring match on cmd
shlog search --cwd ~/dev/myproject   # filter by working directory
shlog search --exit 1                # show only failures
shlog search --since "1h"            # last hour
shlog search --since "2026-03-29"    # since specific date

# Activation
shlog activate                       # modifies ~/.zshrc
shlog deactivate                     # reverts ~/.zshrc

# Per-session wrap
shlog wrap -- claude                 # wrap a specific command

# Status
shlog status                         # is shlog active? log size? location?

# Config
shlog config shell /bin/zsh          # set real shell path
```

### Output Format

**Default:** Human-readable table.

```
TIMESTAMP            CMD                          EXIT  DURATION  CWD
2026-03-30 14:32:01  git push origin main         0     1.4s      ~/dev/myproject
2026-03-30 14:31:58  git add -A                   0     0.1s      ~/dev/myproject
2026-03-30 14:31:55  cargo test                   1     12.3s     ~/dev/myproject
```

**`--json` flag:** Raw JSONL passthrough for scripting and piping.

## Configuration

**File:** `~/.shlog/config.toml`

```toml
# The real shell to delegate to
shell = "/bin/zsh"

# Log file location
log_path = "~/.shlog/logs/commands.jsonl"
```

### Activation Mechanism

`shlog activate` appends a marked block to `~/.zshrc`:

```bash
# >>> shlog >>>
export SHLOG_REAL_SHELL="/bin/zsh"
export SHELL="$HOME/.cargo/bin/shlog"
# <<< shlog <<<
```

`shlog deactivate` removes this block.

The user must open a new terminal tab for changes to take effect.

### Install Location

`cargo install` places the binary in `~/.cargo/bin/shlog`. The activate command references `$HOME/.cargo/bin/shlog`. Homebrew distribution is a future option.

## Dependencies

### Shim path (performance-critical)
- `std` only — no external crates

### CLI path
- `clap` — argument parsing
- `serde` / `serde_json` — JSON serialization and deserialization
- `chrono` — timestamp parsing and human-friendly duration formatting

## Open Research Items

### Session Detection

Multiple concurrent Claude Code sessions (e.g., different terminal tabs) all write to the same log. Differentiating them is valuable for debugging but non-trivial without hooks.

Research vectors:
- `$TERM_SESSION_ID` — some terminal emulators set this per-tab
- TTY device (`/dev/ttys*`) — unique per terminal session
- Process group / session leader PID
- Timing-based heuristics (gaps between commands)
- `ppid` correlation (commands from the same parent)

**Decision:** Deferred past POC. The `ppid` field is captured in logs to support future session grouping. This needs a spike to determine which signals are reliable across common macOS terminal emulators.

### Log Rotation

Single file works for early usage. Strategy needed when logs grow large:
- Daily rotation (`commands-2026-03-30.jsonl`)
- Size-based rotation
- Retention policy

**Decision:** Deferred past POC. JSONL format makes any rotation strategy straightforward to add later.

## Future Work (Not in POC)

- **Web dashboard** — browser-based visualization with filtering, timeline view, most common commands, flagged "dangerous" commands
- **Session grouping** — per-session views once session detection is solved
- **PATH shims** — optional per-tool shims for direct `execvp` coverage
- **Linux support**
- **Log export** — CSV, SQLite, etc.
- **Homebrew formula**
