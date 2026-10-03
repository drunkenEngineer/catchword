#!/bin/sh
# Privacy invariant: the engine and the store must not depend on any network library.
set -e
for crate in catchword-engine catchword-store; do
  if cargo tree -p "$crate" --edges normal --prefix none | grep -Eiw "reqwest|hyper|ureq|curl|isahc|surf|tokio|async-std|socket2|mio|tungstenite|h2|quinn|rustls|native-tls|openssl"; then
    echo "FAIL: $crate depends on a network library (listed above)." >&2
    exit 1
  fi
done
echo "OK: no network library in the engine or the store."
