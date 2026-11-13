#!/bin/sh
# Starts the service named by IH_SERVICE. Extra arguments go to the binary,
# so `docker run ... inference-hub --once` runs the worker once.
#
# With RUN_SEED=true the worker first imports the curated data (`data/`), so a
# deploy that changes it never leaves the market adapters pointing at
# organizations that are not in the database yet. The import is idempotent, waits
# up to two minutes for Postgres and applies migrations itself. If it fails, the
# worker starts anyway on the data it has and the failure is logged.
set -eu

bin="${IH_BIN_DIR:-/app/bin}"
data="${IH_DATA_DIR:-/app/data}"

case "${IH_SERVICE:-api}" in
  api)    exec "$bin/api" "$@" ;;
  worker)
    if [ "${RUN_SEED:-false}" = "true" ]; then
      echo "RUN_SEED=true: importing the curated data" >&2
      if "$bin/seed" "$data"; then
        echo "curated data imported" >&2
      else
        echo "seed failed (exit $?); starting the worker on the data already in the database" >&2
      fi
    fi
    exec "$bin/worker" "$@"
    ;;
  seed)   exec "$bin/seed" "$@" "$data" ;;
  *)
    echo "IH_SERVICE must be api, worker or seed (got '${IH_SERVICE}')" >&2
    exit 64
    ;;
esac
