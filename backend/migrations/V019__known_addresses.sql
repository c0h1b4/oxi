-- Denormalized table of known email addresses extracted from message headers.
-- Avoids expensive json_each() scanning of to_addresses/cc_addresses columns.
CREATE TABLE IF NOT EXISTS known_addresses (
    email      TEXT NOT NULL,
    name       TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (email)
);

CREATE INDEX IF NOT EXISTS idx_known_addresses_name ON known_addresses(name);

-- Backfill from existing messages: from_address
INSERT INTO known_addresses (email, name)
SELECT from_address, COALESCE(from_name, '')
FROM messages
WHERE from_address != ''
ON CONFLICT(email) DO UPDATE SET name = excluded.name WHERE excluded.name != '';

-- Backfill from existing messages: to_addresses (JSON array)
INSERT INTO known_addresses (email, name)
SELECT value->>'address', COALESCE(value->>'name', '')
FROM messages, json_each(to_addresses)
WHERE json_valid(to_addresses)
  AND value->>'address' IS NOT NULL
  AND value->>'address' != ''
ON CONFLICT(email) DO UPDATE SET name = excluded.name WHERE excluded.name != '';

-- Backfill from existing messages: cc_addresses (JSON array)
INSERT INTO known_addresses (email, name)
SELECT value->>'address', COALESCE(value->>'name', '')
FROM messages, json_each(cc_addresses)
WHERE json_valid(cc_addresses)
  AND value->>'address' IS NOT NULL
  AND value->>'address' != ''
ON CONFLICT(email) DO UPDATE SET name = excluded.name WHERE excluded.name != '';
