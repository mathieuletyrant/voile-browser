#!/usr/bin/env bash
# Build de dev puis lancement, sur un profil à part (target/dev-profile).
# La sidebar est lue depuis ui/ : ⌘R la recharge sans recompiler.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo run -q -p voile-bundle -- -o target/bundle

bin=target/bundle/Voile.app/Contents/MacOS/voile
[ -x "$bin" ] || bin=target/bundle/voile

# --use-mock-keychain : la signature ad hoc change à chaque build, macOS
# redemanderait sinon l'accès au trousseau à chaque lancement.
VOILE_DATA_DIR="${VOILE_DATA_DIR:-$PWD/target/dev-profile}" VOILE_UI_DIR="$PWD/ui" \
  exec "$bin" --use-mock-keychain "$@"
