#!/bin/sh
# Privacy invariant: the engine, the store, the extraction worker, the
# embedding runtime and the service must not
# depend on any network library.
set -e
for crate in catchword-engine catchword-store catchword-worker catchword-embed catchword-service; do
  if cargo tree -p "$crate" --edges normal --prefix none | grep -Eiw "reqwest|hyper|ureq|curl|isahc|surf|tokio|async-std|socket2|mio|tungstenite|h2|quinn|rustls|native-tls|openssl"; then
    echo "FAIL: $crate depends on a network library (listed above)." >&2
    exit 1
  fi
done
echo "OK: no network library in the engine, store, worker, embedding runtime or service."

# The desktop shell may hold network code only in the GitHub build, for the
# updater (ADR-24). Without that feature, as in the Store build, it has no
# web client at all. (Tauri itself runs on tokio, so that is not listed.)
if cargo tree -p catchword-desktop --edges normal --prefix none | grep -Eiw "reqwest|hyper|ureq|curl|isahc|surf|tungstenite|h2|quinn|rustls|native-tls|openssl"; then
  echo "FAIL: the desktop shell without the updater depends on a network library (listed above)." >&2
  exit 1
fi
echo "OK: no network library in the desktop shell without the updater (the Store build)."
