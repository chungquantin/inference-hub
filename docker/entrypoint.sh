#!/bin/sh
# Starts the service named by IH_SERVICE. Extra arguments go to the binary,
# so `docker run ... inference-hub --once` runs the worker once.
set -eu

case "${IH_SERVICE:-api}" in
  api)    exec /app/bin/api "$@" ;;
  worker) exec /app/bin/worker "$@" ;;
  seed)   exec /app/bin/seed "$@" /app/data ;;
  *)
    echo "IH_SERVICE must be api, worker or seed (got '${IH_SERVICE}')" >&2
    exit 64
    ;;
esac
