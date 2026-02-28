# Epic 5: Real-Time Sync & Notifications — Design

**Date:** 2026-02-28
**Status:** Approved
**Goal:** Complete the remaining Epic 5 stories: connection status UI, polling fallback, browser notifications, and notification preferences.

## What Already Exists

The core real-time pipeline is fully implemented on `main`:

- **Backend:** WebSocket handler at `GET /api/ws`, IMAP IDLE manager with auto-reconnect + exponential backoff, EventBus with per-user broadcast channels, 30s keepalive pings
- **Frontend:** `useWebSocket` hook with exponential backoff reconnection, TanStack Query cache invalidation on `NewMessages`/`FlagsChanged`/`FolderUpdated` events
- **Key files:** `backend/src/realtime/{ws.rs, idle.rs, events.rs}`, `frontend/src/hooks/useWebSocket.ts`

## Remaining Work

### 1. Connection Status Indicator (Story 5.3)

**Location:** Bottom of NavRail, near settings/logout buttons.

**Behavior:**
- Green dot = `connected` — tooltip "Connected"
- Amber pulsing dot = `connecting` — tooltip "Reconnecting..."
- Red dot = `disconnected` (after 3+ failed reconnects) — tooltip "Disconnected"
- `aria-live="polite"` for screen reader announcements on status changes

**Implementation:**
- Capture `useWebSocket()` return value in `MailPage`, pass as prop to `NavRail`
- Add `ConnectionStatus` component to NavRail bottom section
- Track consecutive failures in `useWebSocket` to distinguish "reconnecting" from "disconnected"

### 2. Polling Fallback (Story 5.3)

**Trigger:** WebSocket stays `disconnected` for >60 seconds.

**Behavior:**
- Poll `GET /api/folders` every 30 seconds via React Query's `refetchInterval`
- When WebSocket reconnects, disable polling automatically
- No separate hook needed — use React Query's built-in `refetchInterval` option, toggled by WebSocket status

**Implementation:**
- In `useMessages` and `useFolders` hooks, accept a `pollingEnabled` flag
- Set `refetchInterval: pollingEnabled ? 30000 : false`
- `MailPage` derives `pollingEnabled = status === "disconnected"`

### 3. Browser Notifications (Story 5.4)

**Hook:** `useNotifications()`

**Permission flow:**
1. On mount, check `Notification.permission`
2. If `"default"`, show a dismissible in-app banner: "Enable desktop notifications to get alerted about new emails" with "Enable" and "Not now" buttons
3. On "Enable" click, call `Notification.requestPermission()`
4. If `"denied"`, hide banner, disable notification features

**Event integration:**
- Add `onEvent` callback parameter to `useWebSocket`
- `useNotifications` provides the callback
- On `NewMessages` event, if `document.hidden` (tab not focused) and permission granted: `new Notification(subject, { body: senderName, tag: messageId })`
- On notification click: `window.focus()`, navigate to folder + select message

### 4. Notification Preferences (Story 5.5)

**Backend:**

New migration (`V006__notification_preferences.sql`):
```sql
CREATE TABLE IF NOT EXISTS notification_preferences (
    id INTEGER PRIMARY KEY DEFAULT 1,
    enabled INTEGER NOT NULL DEFAULT 1,
    sound INTEGER NOT NULL DEFAULT 0,
    folders TEXT NOT NULL DEFAULT '["INBOX"]',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Endpoints:
- `GET /api/settings/notifications` — returns current preferences
- `PUT /api/settings/notifications` — updates preferences

**Frontend:**

Add "Notifications" section to `SettingsPanel`:
- Global enable/disable toggle
- Sound on/off toggle
- Folder multi-select (which folders trigger notifications, default: INBOX only)
- All changes auto-saved (same pattern as identity settings)

Store preferences in React Query cache (fetched on mount, invalidated on update). The `useNotifications` hook reads from this cache to decide whether to fire notifications.

## Critical Files

| Purpose | File |
|---------|------|
| WebSocket hook (modify) | `frontend/src/hooks/useWebSocket.ts` |
| Main page (modify) | `frontend/src/app/(auth)/mail/page.tsx` |
| NavRail (modify) | `frontend/src/components/shared/NavRail.tsx` |
| Settings panel (modify) | `frontend/src/components/settings/SettingsPanel.tsx` |
| Notifications hook (create) | `frontend/src/hooks/useNotifications.ts` |
| Notification settings component (create) | `frontend/src/components/settings/NotificationSettings.tsx` |
| Notification preferences DB (create) | `backend/src/db/notification_preferences.rs` |
| Notification preferences routes (create) | `backend/src/routes/notification_preferences.rs` |
| Migration (create) | `backend/migrations/V006__notification_preferences.sql` |

## Verification

1. **Backend tests:** `cargo test` — notification preferences CRUD tests pass
2. **Frontend build:** `cd frontend && bun run build` — no type errors
3. **Manual E2E:** Send email to self → browser notification appears when tab unfocused → clicking notification focuses tab and navigates to email
4. **Reconnection:** Kill backend → status dot turns amber then red → restart backend → status returns to green, polling stops
5. **Settings:** Toggle notifications off → no browser notifications fire → toggle back on → notifications resume
