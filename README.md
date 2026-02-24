# oxi

Modern, fast, and secure open-source webmail client built with React + Rust.

A feature-complete replacement for Roundcube that connects to any existing IMAP/SMTP mail server. Not a mail platform — just a clean, modern webmail UI.

## Status

**Under development** — not yet ready for use.

## Planned Stack

- **Frontend**: Next.js, Shadcn/ui, Tailwind CSS
- **Backend**: Rust (Axum)
- **Protocols**: IMAP (read), SMTP (send), Sieve (filters)
- **Database**: SQLite per user (local cache)
- **Search**: Tantivy (Rust-native full-text search)
- **Auth**: IMAP credentials (login with email+password, authenticated against mail server)
- **Deploy**: Docker image, connects to any IMAP/SMTP server

## Why?

- Roundcube works but looks dated (PHP, jQuery UI)
- Alternatives are either PHP-based, ugly, or full platforms that bundle their own mail server
- No modern webmail client exists with a React frontend and a fast/secure Rust backend

## License

MIT
