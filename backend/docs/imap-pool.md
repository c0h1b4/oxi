# Request-path IMAP reuse

`RealImapClient` owns a pool shared by all request tasks using that client.
The identity includes host, port, TLS, email and password, stored as a SHA-256
digest of length-prefixed fields. Credential changes cannot reuse old sessions.

Each identity has at most three pooled connections, including connections being
established or exclusively leased. Checkout waits at most ten seconds for a slot.
Reused sessions must pass a ten-second NOOP; failed validation closes the socket
and checkout opens a replacement. TCP/TLS/LOGIN establishment is bounded by ten
seconds. Only a fully consumed, successful operation explicitly returns its
session. Errors and cancellation close it, so partial responses cannot leak into
another operation. No request operation is automatically retried.

Idle sessions expire after two minutes, checked on checkout and by a sweep every
30 seconds. The sweep removes unused identity entries too. Closing sockets avoids
IMAP CLOSE, which would expunge deleted messages. Selected-mailbox operations
always SELECT their own folder; STATUS and LIST do not rely on prior selection.

Pooled paths: LIST, SELECT status, extended STATUS, headers, body, UID/flag
reconciliation and CONDSTORE changed flags. The public trait and HTTP API remain
unchanged. Dedicated realtime IDLE sessions still use `connect` directly.

Follow-up: pool mutations after auditing MOVE/UID EXPUNGE fallbacks that catch
protocol errors, and pool quota/folder-size reads after making their raw-response
and timeout paths explicitly discard incomplete sessions. These paths retain
fresh connections and logout in this change.

Tests use a local plaintext fake IMAP server and the actual async-imap client to
verify login reuse, error disposal, failed NOOP recovery, exclusive checkout
bounds, abandoned leases, credential isolation and stale-session eviction.
