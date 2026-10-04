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
reconciliation, CONDSTORE changed flags, quota and folder-size reads. The public
trait and HTTP API remain unchanged. Dedicated realtime IDLE sessions still use `connect` directly.

Follow-up: pool mutations after auditing MOVE/UID EXPUNGE fallbacks that catch
protocol errors. Mutations retain fresh connections and logout.

Quota reads return sessions only after a matching tagged OK, including an empty
quota result. Unsupported QUOTA (NO/BAD) returns None but discards the lease.
Errors, EOF and quota timeouts discard it too. Folder-size reads reuse sessions
after successful FETCH completion (or an empty SELECT); a timed-out FETCH retains
the existing partial-size result but discards the unfinished session.

Debug logs include a short credential-identity digest prefix and checkout/return
actions (connect, reuse, discard, recycle), without logging credentials.

Tests use a local plaintext fake IMAP server and the actual async-imap client to
verify login reuse, error disposal, failed NOOP recovery, exclusive checkout
bounds, abandoned leases, credential isolation and stale-session eviction.

An ignored TLS regression can be run against a real server with credentials in
`TEST_IMAP_HOST`, `TEST_IMAP_EMAIL`, and `TEST_IMAP_PASSWORD`:
`cargo test real_tls_reads_reuse_one_login -- --ignored --nocapture`.
It asserts one connection for repeated LIST/SELECT/header/quota/size operations
on a server that returns tagged OK for quota requests (including no quota).

The original read-path pool did reuse TLS sessions, but omitted quota and size
reads. A server returning OK without quota data sends `/api/quota` into its
per-folder size fallback, opening and logging out one fresh connection per
folder while ordinary pooled sessions remain open. Regression coverage now
includes that fallback rather than only LIST and SELECT operations.
