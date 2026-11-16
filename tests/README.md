# Runtime base-path regression checks

Build one placeholder export and the backend, then reuse them for every prefix:

```sh
(cd frontend && NEXT_PUBLIC_BASE_PATH=/__OXI_BASE_PATH__ bun run build)
(cd backend && cargo build --locked)
python3 tests/check-runtime-base-path.py backend/target/debug/oxi-email-server frontend/out
```

The Python check uses only the standard library. It checks unset/empty/root,
`/oxi`, `/mail`, and `/apps/mail/`, all generated text templates, assets, redirects, API
routes, input validation, source immutability, and temporary-export cleanup on
SIGTERM and SIGINT.

The browser check uses Playwright installed outside the project (the CI workflow
installs the pinned version). To run with system Chromium:

```sh
npm install --prefix /tmp/oxi-browser --no-package-lock playwright@1.64.1
PLAYWRIGHT_MODULE=/tmp/oxi-browser/node_modules/playwright \
CHROMIUM_EXECUTABLE=/usr/bin/chromium \
node tests/check-runtime-navigation.cjs backend/target/debug/oxi-email-server frontend/out
```

Omit `CHROMIUM_EXECUTABLE` to use Playwright's installed browser. The test starts
its own loopback IMAP fixture and backend. It checks hydration, direct loads,
client-side login/logout navigation, cookie paths, assets, and authenticated
WebSocket upgrades at root and three non-root prefixes. No real mail credentials
or external mail server are used. Pass `/bin/false` as the backend to verify an
early backend failure exits promptly with a failed startup assertion.
