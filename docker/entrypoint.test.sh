#!/bin/sh
# Tests docker/entrypoint.sh with stand-in binaries: no Docker, no database.
#   sh docker/entrypoint.test.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin" "$tmp/data"

# Each stand-in records that it ran, with its arguments, in order.
for name in api worker seed; do
  cat > "$tmp/bin/$name" <<EOF
#!/bin/sh
echo "$name \$*" >> "$tmp/calls"
[ "$name" = seed ] && [ -n "\${SEED_FAILS:-}" ] && exit 3
exit 0
EOF
  chmod +x "$tmp/bin/$name"
done

run() { : > "$tmp/calls"; env IH_BIN_DIR="$tmp/bin" IH_DATA_DIR="$tmp/data" "$@" sh "$here/entrypoint.sh" $ARGS 2> "$tmp/err" || true; }
expect() {
  want=$1; shift
  got=$(tr '\n' ';' < "$tmp/calls")
  if [ "$got" != "$want" ]; then
    echo "FAIL: $*" >&2; echo "  want: $want" >&2; echo "  got:  $got" >&2; exit 1
  fi
}

ARGS=""
run IH_SERVICE=worker;                        expect "worker ;" "worker without RUN_SEED does not seed"
run IH_SERVICE=worker RUN_SEED=false;         expect "worker ;" "RUN_SEED=false does not seed"
run IH_SERVICE=worker RUN_SEED=true;          expect "seed $tmp/data;worker ;" "RUN_SEED=true seeds first, then starts the worker"
run IH_SERVICE=worker RUN_SEED=true SEED_FAILS=1
expect "seed $tmp/data;worker ;" "a failing seed still starts the worker"
grep -q "seed failed (exit 3)" "$tmp/err" || { echo "FAIL: a failing seed is not logged" >&2; exit 1; }
run IH_SERVICE=api RUN_SEED=true;             expect "api ;" "the API never seeds"
ARGS="--once"
run IH_SERVICE=worker RUN_SEED=true;          expect "seed $tmp/data;worker --once;" "arguments reach the worker, not the seed"
ARGS=""
run IH_SERVICE=seed;                          expect "seed $tmp/data;" "the seed service imports and exits"
run IH_SERVICE=nope
[ -s "$tmp/calls" ] && { echo "FAIL: an unknown service ran something" >&2; exit 1; }
echo "entrypoint: all cases pass"
