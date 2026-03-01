# Epic 5: Real-Time Sync & Notifications — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete the remaining Epic 5 stories — connection status indicator, smart polling fallback, browser notifications, and notification preferences.

**Architecture:** The core real-time pipeline (IMAP IDLE → EventBus → WebSocket → React Query cache invalidation) is already working. This plan adds the user-facing layer: a connection status dot in the NavRail, browser Notification API integration, and a backend-persisted notification preferences system. The WebSocket hook gains an `onEvent` callback so the notifications hook can react to events. Polling is already present (`useFolders` at 30s, `useMessages` at 60s) and will be made WebSocket-aware: poll only when disconnected.

**Tech Stack:** Browser Notification API, React Query, Zustand, Axum (Rust), rusqlite

---

## Phase 1: Enhance WebSocket Hook + Connection Status UI

### Task 1.1: Add `onEvent` callback and failure count to `useWebSocket`

**Modify:** `frontend/src/hooks/useWebSocket.ts`

The hook needs two enhancements:
1. Accept an `onEvent` callback so `useNotifications` (Phase 3) can receive raw events
2. Track consecutive reconnect failures so the UI can distinguish "reconnecting" from "disconnected"

**Step 1: Update the hook**

```typescript
"use client";

import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

export type WsStatus = "connected" | "connecting" | "disconnected";

export interface MailEvent {
  type: "NewMessages" | "FlagsChanged" | "FolderUpdated";
  data?: { folder: string };
}

/**
 * Connects to the backend WebSocket endpoint for real-time mail events.
 * Automatically reconnects with exponential backoff on disconnect.
 * Invalidates React Query caches when events arrive.
 *
 * @param onEvent - optional callback invoked for every parsed MailEvent
 */
export function useWebSocket(onEvent?: (event: MailEvent) => void) {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<WsStatus>("disconnected");
  const [failCount, setFailCount] = useState(0);
  const wsRef = useRef<WebSocket | null>(null);
  const backoffRef = useRef(1000);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mountedRef = useRef(true);
  const connectRef = useRef<(() => void) | null>(null);
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  useEffect(() => {
    mountedRef.current = true;

    const connect = () => {
      if (!mountedRef.current) return;

      setStatus("connecting");

      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const ws = new WebSocket(`${protocol}//${window.location.host}/api/ws`);
      wsRef.current = ws;

      ws.onopen = () => {
        if (!mountedRef.current) return;
        setStatus("connected");
        setFailCount(0);
        backoffRef.current = 1000;
      };

      ws.onmessage = (event) => {
        try {
          const mailEvent: MailEvent = JSON.parse(event.data);

          // Notify external listener (e.g. useNotifications)
          onEventRef.current?.(mailEvent);

          switch (mailEvent.type) {
            case "NewMessages":
              if (mailEvent.data?.folder) {
                queryClient.invalidateQueries({
                  queryKey: ["messages", mailEvent.data.folder],
                });
              }
              queryClient.invalidateQueries({ queryKey: ["folders"] });
              break;
            case "FlagsChanged":
              if (mailEvent.data?.folder) {
                queryClient.invalidateQueries({
                  queryKey: ["messages", mailEvent.data.folder],
                });
              }
              queryClient.invalidateQueries({ queryKey: ["folders"] });
              break;
            case "FolderUpdated":
              queryClient.invalidateQueries({ queryKey: ["folders"] });
              break;
          }
        } catch {
          // Ignore malformed messages
        }
      };

      ws.onclose = () => {
        if (!mountedRef.current) return;
        setStatus("disconnected");
        setFailCount((c) => c + 1);
        wsRef.current = null;

        const delay = backoffRef.current;
        backoffRef.current = Math.min(delay * 2, 30000);
        reconnectTimerRef.current = setTimeout(() => connectRef.current?.(), delay);
      };

      ws.onerror = () => {
        // onclose will fire after onerror, which handles reconnection
      };
    };

    connectRef.current = connect;
    connect();

    return () => {
      mountedRef.current = false;
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [queryClient]);

  return { status, failCount };
}
```

**Step 2: Verify frontend builds**

Run: `cd frontend && bun run build`
Expected: No type errors. The return type changed from `WsStatus` to `{ status, failCount }`, so `MailPage` will need an update (Task 1.2).

**Step 3: Commit**

```bash
git add frontend/src/hooks/useWebSocket.ts
git commit -m "feat(ws): add onEvent callback and failure tracking to useWebSocket"
```

---

### Task 1.2: Create `ConnectionStatus` component and wire into NavRail

**Create:** `frontend/src/components/shared/ConnectionStatus.tsx`
**Modify:** `frontend/src/components/shared/NavRail.tsx`
**Modify:** `frontend/src/app/(auth)/mail/page.tsx`

**Step 1: Create the ConnectionStatus component**

```tsx
// frontend/src/components/shared/ConnectionStatus.tsx
"use client";

import type { WsStatus } from "@/hooks/useWebSocket";
import { cn } from "@/lib/utils";

interface ConnectionStatusProps {
  status: WsStatus;
  failCount: number;
}

export function ConnectionStatus({ status, failCount }: ConnectionStatusProps) {
  const isReconnecting = status === "connecting" || (status === "disconnected" && failCount < 3);
  const isFailed = status === "disconnected" && failCount >= 3;
  const isConnected = status === "connected";

  const label = isConnected
    ? "Connected"
    : isReconnecting
      ? "Reconnecting..."
      : "Disconnected";

  return (
    <div className="flex items-center justify-center py-2" title={label}>
      <span
        role="status"
        aria-live="polite"
        aria-label={label}
        className={cn(
          "size-2 rounded-full",
          isConnected && "bg-green-500",
          isReconnecting && "animate-pulse bg-amber-500",
          isFailed && "bg-red-500",
        )}
      />
    </div>
  );
}
```

**Step 2: Update NavRail to accept and render connection status**

In `NavRail.tsx`, add a `wsStatus` and `wsFailCount` prop, render `<ConnectionStatus>` above the bottom action buttons.

```tsx
// Add to NavRail props:
import { ConnectionStatus } from "./ConnectionStatus";
import type { WsStatus } from "@/hooks/useWebSocket";

// Change the export to accept props:
export function NavRail({
  wsStatus = "disconnected",
  wsFailCount = 0,
}: {
  wsStatus?: WsStatus;
  wsFailCount?: number;
}) {
  // ... existing code ...

  // In the render, add before the bottom actions div:
  // <ConnectionStatus status={wsStatus} failCount={wsFailCount} />
```

Specifically in the JSX, insert `<ConnectionStatus>` between `{/* Spacer */}` and `{/* Bottom actions */}`:

```tsx
      {/* Spacer */}
      <div className="flex-1" />

      <ConnectionStatus status={wsStatus} failCount={wsFailCount} />

      {/* Bottom actions */}
```

**Step 3: Update MailPage to capture and pass WebSocket status**

In `MailPage`, change `useWebSocket()` call to destructure the return value and pass it to `NavRail`:

```tsx
export default function MailPage() {
  const viewMode = useUiStore((s) => s.viewMode);
  useKeyboardShortcuts();
  const { status: wsStatus, failCount: wsFailCount } = useWebSocket();

  // In each viewMode return, pass props to NavRail:
  // <NavRail wsStatus={wsStatus} wsFailCount={wsFailCount} />
```

All three render paths (contacts, settings, mail) must pass these props to `<NavRail>`.

**Step 4: Verify frontend builds**

Run: `cd frontend && bun run build`
Expected: No type errors.

**Step 5: Commit**

```bash
git add frontend/src/components/shared/ConnectionStatus.tsx \
       frontend/src/components/shared/NavRail.tsx \
       frontend/src/app/\(auth\)/mail/page.tsx
git commit -m "feat(ui): add connection status indicator to NavRail"
```

---

### Task 1.3: Make polling WebSocket-aware

**Modify:** `frontend/src/hooks/useFolders.ts`
**Modify:** `frontend/src/hooks/useMessages.ts`
**Modify:** `frontend/src/app/(auth)/mail/page.tsx`

Currently `useFolders` polls every 30s and `useMessages` every 60s, regardless of WebSocket status. Change them so:
- When WebSocket is `connected`: disable polling (events handle updates)
- When WebSocket is `disconnected`: keep the existing polling intervals

**Step 1: Store WebSocket status globally**

Rather than threading props through every component, create a simple React context. Add to `MailPage`:

```tsx
// frontend/src/lib/ws-context.ts
"use client";

import { createContext, useContext } from "react";
import type { WsStatus } from "@/hooks/useWebSocket";

interface WsContextValue {
  status: WsStatus;
  failCount: number;
}

export const WsContext = createContext<WsContextValue>({
  status: "disconnected",
  failCount: 0,
});

export function useWsStatus() {
  return useContext(WsContext);
}
```

In `MailPage`, wrap content with `<WsContext.Provider value={{ status: wsStatus, failCount: wsFailCount }}>`.

**Step 2: Update `useFolders`**

```typescript
export function useFolders() {
  const { status } = useWsStatus();
  return useQuery({
    queryKey: ["folders"],
    queryFn: () => apiGet<FoldersResponse>("/folders"),
    refetchInterval: status === "connected" ? false : 30_000,
  });
}
```

**Step 3: Update `useMessages`**

```typescript
export function useMessages(folder: string) {
  const { status } = useWsStatus();
  return useInfiniteQuery({
    // ... existing config ...
    refetchInterval: status === "connected" ? false : 60_000,
  });
}
```

**Step 4: Verify frontend builds**

Run: `cd frontend && bun run build`
Expected: No type errors.

**Step 5: Commit**

```bash
git add frontend/src/lib/ws-context.ts \
       frontend/src/hooks/useFolders.ts \
       frontend/src/hooks/useMessages.ts \
       frontend/src/app/\(auth\)/mail/page.tsx
git commit -m "feat(polling): disable polling when WebSocket is connected"
```

---

## Phase 2: Notification Preferences Backend

### Task 2.1: Add V009 migration for notification_preferences

**Create:** `backend/migrations/V009__notification_preferences.sql`
**Modify:** `backend/src/db/pool.rs` — add entry `(9, include_str!(...))` to `MIGRATIONS` array

**Step 1: Create the migration file**

```sql
-- V009: Notification preferences (singleton row per user DB)
CREATE TABLE IF NOT EXISTS notification_preferences (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 1,
    sound INTEGER NOT NULL DEFAULT 0,
    folders TEXT NOT NULL DEFAULT '["INBOX"]',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert the default row
INSERT OR IGNORE INTO notification_preferences (id) VALUES (1);
```

The `CHECK (id = 1)` constraint ensures only one row — this is a singleton settings table.

**Step 2: Register the migration**

In `backend/src/db/pool.rs`, add to the `MIGRATIONS` array:

```rust
    (9, include_str!("../../migrations/V009__notification_preferences.sql")),
```

**Step 3: Run tests**

Run: `cd backend && cargo test db::pool`
Expected: All pool tests pass (migrations applied to test DBs).

**Step 4: Commit**

```bash
git add backend/migrations/V009__notification_preferences.sql \
       backend/src/db/pool.rs
git commit -m "feat(db): add V009 migration for notification preferences"
```

---

### Task 2.2: Create `db/notification_preferences.rs` module

**Create:** `backend/src/db/notification_preferences.rs`
**Modify:** `backend/src/db/mod.rs` — add `pub mod notification_preferences;`

Follow the same patterns as `db/identities.rs` (struct, row mapper, CRUD functions, tests).

**Step 1: Write the module**

```rust
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Notification preferences for a user (singleton row).
#[derive(Debug, Clone, Serialize)]
pub struct NotificationPreferences {
    pub enabled: bool,
    pub sound: bool,
    pub folders: Vec<String>,
    pub updated_at: String,
}

/// Input for updating notification preferences.
#[derive(Debug, Deserialize)]
pub struct UpdateNotificationPreferences {
    pub enabled: Option<bool>,
    pub sound: Option<bool>,
    pub folders: Option<Vec<String>>,
}

/// Get the notification preferences. Returns defaults if the row doesn't exist.
pub fn get_preferences(conn: &Connection) -> Result<NotificationPreferences, String> {
    let result = conn.query_row(
        "SELECT enabled, sound, folders, updated_at FROM notification_preferences WHERE id = 1",
        [],
        |row| {
            let enabled_int: i32 = row.get(0)?;
            let sound_int: i32 = row.get(1)?;
            let folders_json: String = row.get(2)?;
            let updated_at: String = row.get(3)?;
            Ok((enabled_int, sound_int, folders_json, updated_at))
        },
    );

    match result {
        Ok((enabled_int, sound_int, folders_json, updated_at)) => {
            let folders: Vec<String> =
                serde_json::from_str(&folders_json).unwrap_or_else(|_| vec!["INBOX".to_string()]);
            Ok(NotificationPreferences {
                enabled: enabled_int != 0,
                sound: sound_int != 0,
                folders,
                updated_at,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(NotificationPreferences {
            enabled: true,
            sound: false,
            folders: vec!["INBOX".to_string()],
            updated_at: String::new(),
        }),
        Err(e) => Err(format!("Failed to get notification preferences: {e}")),
    }
}

/// Update notification preferences. Only provided fields are changed.
pub fn update_preferences(
    conn: &Connection,
    data: &UpdateNotificationPreferences,
) -> Result<NotificationPreferences, String> {
    // Ensure the row exists
    conn.execute(
        "INSERT OR IGNORE INTO notification_preferences (id) VALUES (1)",
        [],
    )
    .map_err(|e| format!("Failed to ensure preferences row: {e}"))?;

    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(enabled) = data.enabled {
        sets.push(format!("enabled = ?{idx}"));
        values.push(Box::new(enabled as i32));
        idx += 1;
    }
    if let Some(sound) = data.sound {
        sets.push(format!("sound = ?{idx}"));
        values.push(Box::new(sound as i32));
        idx += 1;
    }
    if let Some(ref folders) = data.folders {
        let json = serde_json::to_string(folders)
            .map_err(|e| format!("Failed to serialize folders: {e}"))?;
        sets.push(format!("folders = ?{idx}"));
        values.push(Box::new(json));
        idx += 1;
    }

    if !sets.is_empty() {
        sets.push("updated_at = datetime('now')".to_string());
        let set_clause = sets.join(", ");
        let sql = format!("UPDATE notification_preferences SET {set_clause} WHERE id = 1");
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(|e| format!("Failed to update preferences: {e}"))?;
    }

    get_preferences(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn test_get_default_preferences() {
        let conn = open_test_db();
        let prefs = get_preferences(&conn).unwrap();
        assert!(prefs.enabled);
        assert!(!prefs.sound);
        assert_eq!(prefs.folders, vec!["INBOX"]);
    }

    #[test]
    fn test_update_enabled() {
        let conn = open_test_db();
        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: Some(false),
                sound: None,
                folders: None,
            },
        )
        .unwrap();
        assert!(!prefs.enabled);
        assert!(!prefs.sound);
    }

    #[test]
    fn test_update_folders() {
        let conn = open_test_db();
        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: None,
                sound: None,
                folders: Some(vec!["INBOX".to_string(), "Sent".to_string()]),
            },
        )
        .unwrap();
        assert_eq!(prefs.folders, vec!["INBOX", "Sent"]);
    }

    #[test]
    fn test_update_all_fields() {
        let conn = open_test_db();
        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: Some(false),
                sound: Some(true),
                folders: Some(vec!["INBOX".to_string()]),
            },
        )
        .unwrap();
        assert!(!prefs.enabled);
        assert!(prefs.sound);
        assert_eq!(prefs.folders, vec!["INBOX"]);
    }
}
```

**Step 2: Register the module**

In `backend/src/db/mod.rs`, add:

```rust
pub mod notification_preferences;
```

**Step 3: Run tests**

Run: `cd backend && cargo test db::notification_preferences`
Expected: All 4 tests pass.

**Step 4: Commit**

```bash
git add backend/src/db/notification_preferences.rs backend/src/db/mod.rs
git commit -m "feat(db): add notification preferences CRUD module"
```

---

### Task 2.3: Create notification preferences route handlers

**Create:** `backend/src/routes/notification_preferences.rs`
**Modify:** `backend/src/routes/mod.rs`

Follow the same pattern as `routes/identities.rs`.

**Step 1: Write the route handlers**

```rust
use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::db::notification_preferences::UpdateNotificationPreferences;
use crate::error::AppError;

/// `GET /api/settings/notifications` — get notification preferences.
pub async fn get_notification_preferences(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let prefs = db::notification_preferences::get_preferences(&conn)
        .map_err(AppError::InternalError)?;

    Ok(Json(prefs).into_response())
}

/// `PUT /api/settings/notifications` — update notification preferences.
pub async fn update_notification_preferences(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(data): Json<UpdateNotificationPreferences>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let prefs = db::notification_preferences::update_preferences(&conn, &data)
        .map_err(AppError::InternalError)?;

    Ok(Json(prefs).into_response())
}
```

**Step 2: Register routes**

In `backend/src/routes/mod.rs`:

1. Add `pub mod notification_preferences;` at the top with the other module declarations.
2. Add routes to `protected_data` router (after the identities routes):

```rust
        .route(
            "/settings/notifications",
            get(notification_preferences::get_notification_preferences)
                .put(notification_preferences::update_notification_preferences),
        )
```

Also add `put` to the `use axum::routing::{...}` import if not already there.

**Step 3: Run backend tests**

Run: `cd backend && cargo test`
Expected: All tests pass (compile + existing tests).

**Step 4: Commit**

```bash
git add backend/src/routes/notification_preferences.rs backend/src/routes/mod.rs
git commit -m "feat(api): add GET/PUT /api/settings/notifications endpoints"
```

---

## Phase 3: Browser Notifications (Frontend)

### Task 3.1: Create `useNotificationPreferences` hook

**Create:** `frontend/src/hooks/useNotificationPreferences.ts`

Follow the `useIdentities.ts` pattern (React Query + mutation).

**Step 1: Write the hook**

```typescript
"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPut } from "@/lib/api";

export interface NotificationPreferences {
  enabled: boolean;
  sound: boolean;
  folders: string[];
  updated_at: string;
}

interface UpdateNotificationPreferences {
  enabled?: boolean;
  sound?: boolean;
  folders?: string[];
}

export function useNotificationPreferences() {
  return useQuery({
    queryKey: ["notification-preferences"],
    queryFn: () => apiGet<NotificationPreferences>("/settings/notifications"),
  });
}

export function useUpdateNotificationPreferences() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: UpdateNotificationPreferences) =>
      apiPut<NotificationPreferences>(
        "/settings/notifications",
        data as unknown as Record<string, unknown>,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["notification-preferences"] });
    },
  });
}
```

**Step 2: Verify frontend builds**

Run: `cd frontend && bun run build`
Expected: No type errors.

**Step 3: Commit**

```bash
git add frontend/src/hooks/useNotificationPreferences.ts
git commit -m "feat(frontend): add useNotificationPreferences hook"
```

---

### Task 3.2: Create `useNotifications` hook

**Create:** `frontend/src/hooks/useNotifications.ts`

This hook:
1. Manages browser notification permission state
2. Provides a callback for WebSocket events
3. Fires browser notifications for `NewMessages` when the tab is not focused
4. Respects the user's notification preferences

**Step 1: Write the hook**

```typescript
"use client";

import { useCallback, useEffect, useState } from "react";
import { useUiStore } from "@/stores/useUiStore";
import { useNotificationPreferences } from "./useNotificationPreferences";
import type { MailEvent } from "./useWebSocket";

type NotificationPermission = "default" | "granted" | "denied";

/**
 * Manages browser notification permissions and fires desktop notifications
 * for new mail events when the tab is not focused.
 *
 * Returns:
 * - `permission`: current Notification API permission state
 * - `showBanner`: whether to show the "enable notifications" prompt
 * - `requestPermission`: call to trigger the browser permission dialog
 * - `dismissBanner`: hides the banner without requesting permission
 * - `handleEvent`: callback to pass to useWebSocket's onEvent
 */
export function useNotifications() {
  const [permission, setPermission] = useState<NotificationPermission>(() => {
    if (typeof window === "undefined" || !("Notification" in window)) {
      return "denied";
    }
    return Notification.permission as NotificationPermission;
  });
  const [bannerDismissed, setBannerDismissed] = useState(() => {
    if (typeof window === "undefined") return false;
    return localStorage.getItem("oxi-notif-banner-dismissed") === "true";
  });

  const { data: prefs } = useNotificationPreferences();
  const selectMessage = useUiStore((s) => s.selectMessage);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);

  const showBanner = permission === "default" && !bannerDismissed;

  const requestPermission = useCallback(async () => {
    if (!("Notification" in window)) return;
    const result = await Notification.requestPermission();
    setPermission(result as NotificationPermission);
  }, []);

  const dismissBanner = useCallback(() => {
    setBannerDismissed(true);
    localStorage.setItem("oxi-notif-banner-dismissed", "true");
  }, []);

  // Listen for permission changes (e.g. user changes in browser settings)
  useEffect(() => {
    if (!("Notification" in window)) return;
    const check = () => setPermission(Notification.permission as NotificationPermission);
    // Re-check on visibility change (user may have changed setting in another tab)
    document.addEventListener("visibilitychange", check);
    return () => document.removeEventListener("visibilitychange", check);
  }, []);

  const handleEvent = useCallback(
    (event: MailEvent) => {
      // Only fire for NewMessages
      if (event.type !== "NewMessages") return;

      // Only fire when tab is not focused
      if (!document.hidden) return;

      // Check permission
      if (permission !== "granted") return;

      // Check preferences
      if (!prefs?.enabled) return;

      const folder = event.data?.folder;
      if (!folder) return;

      // Check if this folder is in the user's notification list
      if (!prefs.folders.includes(folder)) return;

      const notification = new Notification("New email", {
        body: `New message in ${folder}`,
        tag: `oxi-${folder}-${Date.now()}`,
        icon: "/icon-192.png",
      });

      notification.onclick = () => {
        window.focus();
        setActiveFolder(folder);
        notification.close();
      };
    },
    [permission, prefs, setActiveFolder],
  );

  return {
    permission,
    showBanner,
    requestPermission,
    dismissBanner,
    handleEvent,
  };
}
```

**Step 2: Verify frontend builds**

Run: `cd frontend && bun run build`
Expected: No type errors.

**Step 3: Commit**

```bash
git add frontend/src/hooks/useNotifications.ts
git commit -m "feat(notifications): add useNotifications hook with browser Notification API"
```

---

### Task 3.3: Wire `useNotifications` into `MailPage` and add notification banner

**Modify:** `frontend/src/app/(auth)/mail/page.tsx`

**Step 1: Create the notification permission banner**

**Create:** `frontend/src/components/shared/NotificationBanner.tsx`

```tsx
"use client";

import { Bell, X } from "lucide-react";

interface NotificationBannerProps {
  onEnable: () => void;
  onDismiss: () => void;
}

export function NotificationBanner({ onEnable, onDismiss }: NotificationBannerProps) {
  return (
    <div className="flex items-center gap-3 border-b border-border bg-accent/50 px-4 py-2">
      <Bell className="size-4 shrink-0 text-primary" />
      <p className="flex-1 text-sm text-foreground">
        Enable desktop notifications to get alerted about new emails.
      </p>
      <button
        onClick={onEnable}
        className="shrink-0 rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90"
      >
        Enable
      </button>
      <button
        onClick={onDismiss}
        className="shrink-0 rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label="Dismiss"
      >
        <X className="size-3.5" />
      </button>
    </div>
  );
}
```

**Step 2: Update MailPage to wire everything together**

In `MailPage`:

```tsx
import { useNotifications } from "@/hooks/useNotifications";
import { NotificationBanner } from "@/components/shared/NotificationBanner";
import { WsContext } from "@/lib/ws-context";

export default function MailPage() {
  const viewMode = useUiStore((s) => s.viewMode);
  useKeyboardShortcuts();

  const { showBanner, requestPermission, dismissBanner, handleEvent } = useNotifications();
  const { status: wsStatus, failCount: wsFailCount } = useWebSocket(handleEvent);

  // ... viewMode checks with NavRail props + WsContext.Provider wrapping ...
  // Add <NotificationBanner> above each view when showBanner is true
}
```

Each render path should wrap content in `<WsContext.Provider>` and conditionally render `<NotificationBanner>` at the top.

**Step 3: Verify frontend builds**

Run: `cd frontend && bun run build`
Expected: No type errors.

**Step 4: Commit**

```bash
git add frontend/src/components/shared/NotificationBanner.tsx \
       frontend/src/app/\(auth\)/mail/page.tsx
git commit -m "feat(notifications): wire browser notifications into MailPage with permission banner"
```

---

## Phase 4: Notification Settings UI

### Task 4.1: Create `NotificationSettings` component

**Create:** `frontend/src/components/settings/NotificationSettings.tsx`
**Modify:** `frontend/src/components/settings/SettingsPanel.tsx`

Follow the same visual pattern as `IdentitySettings.tsx`.

**Step 1: Write the component**

```tsx
"use client";

import { useCallback } from "react";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";
import {
  useNotificationPreferences,
  useUpdateNotificationPreferences,
} from "@/hooks/useNotificationPreferences";
import { useFolders } from "@/hooks/useFolders";
import { cn } from "@/lib/utils";

function Toggle({
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-border p-4">
      <div>
        <div className="text-sm font-medium">{label}</div>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      <button
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        disabled={disabled}
        className={cn(
          "relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors",
          checked ? "bg-primary" : "bg-muted",
          disabled && "cursor-not-allowed opacity-50",
        )}
      >
        <span
          className={cn(
            "pointer-events-none inline-block size-5 rounded-full bg-background shadow-sm ring-0 transition-transform",
            checked ? "translate-x-5" : "translate-x-0",
          )}
        />
      </button>
    </div>
  );
}

export function NotificationSettings() {
  const { data: prefs, isLoading } = useNotificationPreferences();
  const updatePrefs = useUpdateNotificationPreferences();
  const { data: foldersData } = useFolders();

  const browserPermission =
    typeof window !== "undefined" && "Notification" in window
      ? Notification.permission
      : "denied";

  const handleToggle = useCallback(
    (field: "enabled" | "sound", value: boolean) => {
      updatePrefs.mutate(
        { [field]: value },
        {
          onError: (e) => toast.error(`Failed to update: ${e.message}`),
        },
      );
    },
    [updatePrefs],
  );

  const handleFolderToggle = useCallback(
    (folderName: string, add: boolean) => {
      if (!prefs) return;
      const next = add
        ? [...prefs.folders, folderName]
        : prefs.folders.filter((f) => f !== folderName);
      updatePrefs.mutate(
        { folders: next },
        {
          onError: (e) => toast.error(`Failed to update: ${e.message}`),
        },
      );
    },
    [prefs, updatePrefs],
  );

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        Loading notification settings...
      </div>
    );
  }

  if (!prefs) return null;

  const folders = foldersData?.folders ?? [];

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h2 className="text-base font-semibold">Notifications</h2>
        <p className="mt-0.5 text-sm text-muted-foreground">
          Configure desktop notification preferences.
        </p>
      </div>

      {browserPermission === "denied" && (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-700 dark:text-amber-400">
          Browser notifications are blocked. Enable them in your browser
          settings to receive desktop alerts.
        </div>
      )}

      <div className="space-y-3">
        <Toggle
          label="Desktop notifications"
          description="Show browser notifications when new emails arrive"
          checked={prefs.enabled}
          onChange={(v) => handleToggle("enabled", v)}
        />

        <Toggle
          label="Notification sound"
          description="Play a sound when a notification is shown"
          checked={prefs.sound}
          onChange={(v) => handleToggle("sound", v)}
          disabled={!prefs.enabled}
        />
      </div>

      <div>
        <h3 className="mb-2 text-sm font-medium">Notify for folders</h3>
        <p className="mb-3 text-xs text-muted-foreground">
          Choose which folders trigger notifications.
        </p>
        <div className="space-y-1">
          {folders.map((folder) => {
            const isChecked = prefs.folders.includes(folder.name);
            return (
              <label
                key={folder.name}
                className="flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent"
              >
                <input
                  type="checkbox"
                  checked={isChecked}
                  onChange={(e) => handleFolderToggle(folder.name, e.target.checked)}
                  disabled={!prefs.enabled}
                  className="size-4 rounded border-border"
                />
                {folder.name}
              </label>
            );
          })}
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Add to SettingsPanel**

```tsx
// frontend/src/components/settings/SettingsPanel.tsx
"use client";

import { IdentitySettings } from "./IdentitySettings";
import { NotificationSettings } from "./NotificationSettings";

export function SettingsPanel() {
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="border-b border-border px-6 py-4">
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>
      <div className="flex-1 space-y-10 overflow-y-auto p-6">
        <IdentitySettings />
        <NotificationSettings />
      </div>
    </div>
  );
}
```

**Step 3: Verify frontend builds**

Run: `cd frontend && bun run build`
Expected: No type errors.

**Step 4: Commit**

```bash
git add frontend/src/components/settings/NotificationSettings.tsx \
       frontend/src/components/settings/SettingsPanel.tsx
git commit -m "feat(settings): add notification preferences UI"
```

---

## Phase 5: Final Verification

### Task 5.1: Full build and test verification

**Step 1: Backend tests**

Run: `cd backend && cargo test`
Expected: All tests pass including new notification_preferences tests.

**Step 2: Frontend build**

Run: `cd frontend && bun run build`
Expected: No type errors, clean build.

**Step 3: Lint checks**

Run: `cd frontend && bun run lint`
Expected: No lint errors.

**Step 4: Docker build (if applicable)**

Run: `docker build -t oxi-email:epic5 .`
Expected: Image builds successfully.

**Step 5: Commit any remaining fixes and tag**

```bash
git add -A && git commit -m "chore: fix any remaining lint/type issues"
```

---

## Critical Files Reference

| Purpose | File | Action |
|---------|------|--------|
| WebSocket hook | `frontend/src/hooks/useWebSocket.ts` | Modify |
| WS context | `frontend/src/lib/ws-context.ts` | Create |
| Connection status component | `frontend/src/components/shared/ConnectionStatus.tsx` | Create |
| Notification banner | `frontend/src/components/shared/NotificationBanner.tsx` | Create |
| NavRail | `frontend/src/components/shared/NavRail.tsx` | Modify |
| Main page | `frontend/src/app/(auth)/mail/page.tsx` | Modify |
| Folders hook | `frontend/src/hooks/useFolders.ts` | Modify |
| Messages hook | `frontend/src/hooks/useMessages.ts` | Modify |
| Notifications hook | `frontend/src/hooks/useNotifications.ts` | Create |
| Notification prefs hook | `frontend/src/hooks/useNotificationPreferences.ts` | Create |
| Notification settings UI | `frontend/src/components/settings/NotificationSettings.tsx` | Create |
| Settings panel | `frontend/src/components/settings/SettingsPanel.tsx` | Modify |
| DB migration | `backend/migrations/V009__notification_preferences.sql` | Create |
| DB module | `backend/src/db/notification_preferences.rs` | Create |
| DB mod registry | `backend/src/db/mod.rs` | Modify |
| DB pool (migration list) | `backend/src/db/pool.rs` | Modify |
| Route handlers | `backend/src/routes/notification_preferences.rs` | Create |
| Route registry | `backend/src/routes/mod.rs` | Modify |
