# Inference Hub task runner.
# GUI (Next.js web app in apps/web) and Rust (Cargo workspace) live in module
# subdirectories so recipes read as `just gui/dev`, `just rs/format`, etc.
# Run `just gui` or `just rs` to list recipes in that module.

mod gui
mod rs

# Default: list recipes from all modules.
default:
    @just --list --list-submodules

# ── Full local stack ─────────────────────────────────────────────────────────
# State lives under .just/ (gitignored): logs/ for output, pids/ for the
# background service PIDs so dev-down stops exactly what dev-up started.

JUST_DIR := ".just"
LOG_DIR  := JUST_DIR / "logs"
PID_DIR  := JUST_DIR / "pids"

# Bring up Postgres, apply migrations + seed, then start api, web, and worker
# in the background. Logs go to .just/logs/*.log; PIDs to .just/pids/*.pid.
# Re-running restarts the three background services (db is left as-is).
dev-up:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p {{LOG_DIR}} {{PID_DIR}}
    echo "→ starting Postgres…"
    pnpm db:up
    echo "→ running migrations + seed…"
    pnpm seed
    just _bg-start api   "cargo run -p api"
    just _bg-start web   "pnpm --filter web dev"
    just _bg-start worker "cargo run -p worker"
    echo "✓ stack is up"
    echo "  api:    http://127.0.0.1:4000  (docs at /docs)"
    echo "  web:    http://localhost:3000"
    just _dev-status

# Stop the api, web, and worker background services. Postgres stays running.
dev-down:
    #!/usr/bin/env bash
    set -euo pipefail
    for svc in api web worker; do
      pidfile="{{PID_DIR}}/$svc.pid"
      if [[ -f "$pidfile" ]]; then
        pid="$(cat "$pidfile")"
        if kill -0 "$pid" 2>/dev/null; then
          kill "$pid" && echo "→ stopped $svc (pid $pid)"
        else
          echo "→ $svc not running (stale pid $pid)"
        fi
        rm -f "$pidfile"
      else
        echo "→ $svc was not started by dev-up"
      fi
    done

# Tail the background services' logs. Ctrl-C exits the tail, not the services.
dev-logs svc="":
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z "{{svc}}" ]]; then
      tail -F {{LOG_DIR}}/*.log
    else
      tail -F {{LOG_DIR}}/{{svc}}.log
    fi

# Show which background services are running and where their logs/PIDs live.
dev-status:
    @just _dev-status

_dev-status:
    #!/usr/bin/env bash
    set -uo pipefail
    for svc in api web worker; do
      pidfile="{{PID_DIR}}/$svc.pid"
      if [[ -f "$pidfile" ]] && kill -0 "$(cat "$pidfile")" 2>/dev/null; then
        echo "  $svc: running (pid $(cat "$pidfile"), log {{LOG_DIR}}/$svc.log)"
      else
        echo "  $svc: down"
      fi
    done

# Internal: start <name> <command> in the background, recording its PID.
_bg-start name command:
    #!/usr/bin/env bash
    set -euo pipefail
    pidfile="{{PID_DIR}}/{{name}}.pid"
    logfile="{{LOG_DIR}}/{{name}}.log"
    # If already running under a prior dev-up, stop it first.
    if [[ -f "$pidfile" ]] && kill -0 "$(cat "$pidfile")" 2>/dev/null; then
      kill "$(cat "$pidfile")" || true
      sleep 1
    fi
    echo "→ starting {{name}}: {{command}}"
    nohup {{command}} >"$logfile" 2>&1 &
    echo $! >"$pidfile"
