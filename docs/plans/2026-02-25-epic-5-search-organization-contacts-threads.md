# Epic 5: Search, Organization, Contacts & Threads — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add full-text search with Tantivy, folder management (create/rename/delete), contacts/address book with auto-complete, and a proper conversation thread view — the remaining core features for Roundcube parity.

**Architecture:** Backend adds a `search/` module wrapping Tantivy for per-user full-text indexing. New IMAP trait methods for folder CRUD. A `contacts` SQLite table + API for address book. Frontend gets a search bar with filter UI, folder context menu, contacts panel, and an enhanced thread view. All features build on existing patterns (trait-based services, SQLite cache, TanStack Query hooks, Zustand stores).

**Tech Stack:** Tantivy 0.22 (Rust full-text search), async-imap (folder CRUD), rusqlite (contacts), Radix UI (dialogs/menus), TanStack Query, Zustand, Tailwind CSS

---

## Key Design Decisions

1. **Tantivy indexing:** Per-user index stored at `{data_dir}/{user_hash}/tantivy/`. Index on first sync, update incrementally as new messages arrive. Index fields: subject, from, to, body_text, date, folder, uid. Search returns UIDs which are resolved from SQLite cache.
2. **Folder CRUD via IMAP:** Extend `ImapClient` trait with `create_folder`, `rename_folder`, `delete_folder`, `subscribe_folder`. SQLite cache updated after each operation.
3. **Contacts:** Local SQLite `contacts` table (V007 migration). No CardDAV sync for MVP — local-only address book. Auto-complete draws from both contacts table and recently-used addresses extracted from sent messages.
4. **Thread view:** Enhance existing thread detection with proper JWZ-style threading (group by References chain, not just direct in_reply_to). Backend returns full thread with bodies. Frontend renders expandable conversation view.
5. **Search scope:** Search across all folders by default, with optional folder filter. Date range filters handled by Tantivy's date field. Results displayed in a dedicated search results view (replaces message list temporarily).

---

## Phase 1: Full-Text Search with Tantivy

### Task 1.1: Add Tantivy dependency

**Modify:** `backend/Cargo.toml`

Add:
```toml
tantivy = "0.22"
```

**Step:** Add dep, run `cargo check` to verify resolution.

**Commit:** `chore: add tantivy dependency for full-text search`

### Task 1.2: Create `search/` module with indexing engine

**Create:** `backend/src/search/mod.rs`, `backend/src/search/engine.rs`
**Modify:** `backend/src/main.rs` (add `mod search;`)

Define the search engine:

```rust
pub struct SearchEngine {
    base_dir: PathBuf,
}

pub struct UserIndex {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
}

pub struct SearchResult {
    pub uid: u32,
    pub folder: String,
    pub score: f32,
    pub snippet: String, // highlighted snippet from matched field
}

pub struct SearchQuery {
    pub text: String,
    pub folder: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub has_attachment: Option<bool>,
    pub limit: usize,
    pub offset: usize,
}
```

Schema fields:
- `uid` (u64, STORED | INDEXED)
- `folder` (TEXT, STORED | STRING — exact match, not tokenized)
- `subject` (TEXT, STORED | TEXT — full-text)
- `from_address` (TEXT, STORED | STRING)
- `from_name` (TEXT, STORED | TEXT)
- `to_addresses` (TEXT, TEXT)
- `body_text` (TEXT, TEXT — full-text, not stored to save space)
- `date_epoch` (i64, STORED | INDEXED — for range queries)
- `has_attachments` (u64, INDEXED — 0 or 1)

Methods:
- `SearchEngine::new(base_dir)` — constructor
- `SearchEngine::open_user_index(user_hash)` — opens or creates per-user index
- `UserIndex::index_message(uid, folder, subject, from, to, body, date_epoch, has_attachments)` — index a single message
- `UserIndex::index_messages_batch(messages)` — bulk index
- `UserIndex::delete_message(uid, folder)` — remove from index
- `UserIndex::search(query: SearchQuery)` — full-text query, returns `Vec<SearchResult>`
- `UserIndex::reindex_all(conn: &Connection)` — rebuild index from SQLite cache

Use `tantivy::query::BooleanQuery` to combine text search with filters. Use `tantivy::SnippetGenerator` for highlighted snippets.

**Tests:**
- `index_and_search_by_subject`
- `index_and_search_by_body`
- `search_with_folder_filter`
- `search_with_date_range`
- `delete_removes_from_index`
- `reindex_all_from_db`

**Commit:** `feat(search): add Tantivy search engine with per-user indexing`

### Task 1.3: Integrate indexing into message sync flow

**Modify:** `backend/src/routes/messages.rs`

When `list_messages` handler syncs new messages from IMAP and caches them in SQLite, also index them in Tantivy. Add indexing call after `db::messages::upsert_message()`:

```rust
// After syncing headers, index the messages that have body text
// For headers-only sync, index subject + from + to
// Full body indexing happens when fetch_body is called
if let Ok(user_index) = search_engine.open_user_index(&session.user_hash) {
    for header in &new_headers {
        let _ = user_index.index_message(
            header.uid,
            folder,
            &header.subject,
            &header.from_address,
            &header.to_addresses.iter().map(|a| &a.address).collect::<Vec<_>>().join(", "),
            "", // body indexed later on first read
            header.date_epoch,
            header.has_attachments,
        );
    }
}
```

When `get_message` handler fetches and caches the full body, re-index with body text:

```rust
// After caching body, update index with full text
if let Ok(user_index) = search_engine.open_user_index(&session.user_hash) {
    let _ = user_index.index_message(uid, folder, &subject, &from, &to, &body_text, date_epoch, has_attachments);
}
```

**Modify:** `backend/src/routes/mod.rs`
- Add `Extension(Arc<SearchEngine>)` layer to router
- Pass `SearchEngine` to router constructor

**Modify:** `backend/src/main.rs`
- Instantiate `SearchEngine::new(config.data_dir.clone())`
- Pass to `create_router`

**Commit:** `feat(search): integrate Tantivy indexing into message sync flow`

### Task 1.4: Create search API endpoint

**Create:** `backend/src/routes/search.rs`
**Modify:** `backend/src/routes/mod.rs` (add route)

Route: `GET /api/search?q=text&folder=INBOX&from=alice&date_from=2026-01-01&date_to=2026-02-25&has_attachment=true&limit=50&offset=0`

Handler:
1. Parse query parameters into `SearchQuery`
2. Open user index
3. Execute search
4. Resolve UIDs from SQLite cache to get full message headers
5. Return `{ results: MessageHeader[], total_count: number, query: string }`

Handle edge cases:
- Empty query returns validation error
- No index exists yet → return empty results
- Index corruption → log warning, trigger reindex in background, return empty

**Tests:**
- `search_returns_matching_messages`
- `search_with_folder_filter`
- `search_empty_query_returns_400`
- `search_no_results`

**Commit:** `feat(api): add GET /api/search endpoint`

### Task 1.5: Frontend search bar and results UI

**Create:** `frontend/src/hooks/useSearch.ts`
**Create:** `frontend/src/components/mail/SearchBar.tsx`
**Create:** `frontend/src/components/mail/SearchResults.tsx`
**Modify:** `frontend/src/stores/useUiStore.ts` (add search state)
**Modify:** `frontend/src/components/shared/ThreePanelLayout.tsx` (integrate search bar)

**useSearch hook:**
```typescript
export function useSearch(query: string, filters: SearchFilters) {
  return useQuery({
    queryKey: ["search", query, filters],
    queryFn: () => apiGet<SearchResponse>(`/search?${buildSearchParams(query, filters)}`),
    enabled: query.length >= 2,
    keepPreviousData: true,
  });
}
```

**UI store additions:**
```typescript
searchQuery: string;
searchActive: boolean;
searchFilters: SearchFilters;
setSearchQuery: (query: string) => void;
setSearchActive: (active: boolean) => void;
setSearchFilters: (filters: SearchFilters) => void;
clearSearch: () => void;
```

**SearchBar component:**
- Input with magnifying glass icon in the message list header area
- Debounced input (300ms) before triggering search
- Filter toggles: folder dropdown, date range picker, has-attachment checkbox
- `Escape` key clears search and returns to normal message list
- `Cmd/Ctrl+K` keyboard shortcut to focus search bar
- Shows "X results for 'query'" count below input

**SearchResults component:**
- Replaces `MessageList` when `searchActive` is true
- Reuses `MessageListItem` for each result
- Shows folder name badge on each result (since results span folders)
- Highlighted search terms in subject/snippet
- Click navigates to message in its original folder
- Infinite scroll pagination for results
- "No results found" empty state

**Commit:** `feat(frontend): add search bar with debounced query and filter UI`

---

## Phase 2: Folder Management

### Task 2.1: Extend ImapClient trait with folder CRUD

**Modify:** `backend/src/imap/client.rs`

Add to trait:
```rust
async fn create_folder(
    &self, creds: &ImapCredentials, folder_name: &str,
) -> Result<(), ImapError>;

async fn rename_folder(
    &self, creds: &ImapCredentials, from: &str, to: &str,
) -> Result<(), ImapError>;

async fn delete_folder(
    &self, creds: &ImapCredentials, folder_name: &str,
) -> Result<(), ImapError>;

async fn subscribe_folder(
    &self, creds: &ImapCredentials, folder_name: &str, subscribe: bool,
) -> Result<(), ImapError>;
```

**RealImapClient:**
- `create_folder`: `session.create(folder_name)` + `session.subscribe(folder_name)`
- `rename_folder`: `session.rename(from, to)`
- `delete_folder`: `session.delete(folder_name)`
- `subscribe_folder`: `session.subscribe(folder_name)` or `session.unsubscribe(folder_name)`

**MockImapClient:** Record operations in `Mutex<Vec<...>>` for assertion.

**Tests:** Mock-based tests for each operation.

**Commit:** `feat(imap): add folder create/rename/delete/subscribe to ImapClient trait`

### Task 2.2: Folder management API endpoints

**Create:** `backend/src/routes/folder_mgmt.rs`
**Modify:** `backend/src/routes/mod.rs`

Routes:
- `POST /api/folders` — create folder. Body: `{ "name": "Projects" }`
- `PATCH /api/folders/{name}` — rename folder. Body: `{ "new_name": "Work" }`
- `DELETE /api/folders/{name}` — delete folder (with confirmation that it's empty or force flag)
- `PATCH /api/folders/{name}/subscribe` — toggle subscription. Body: `{ "subscribed": true }`

Each handler:
1. Validates input (no empty names, no renaming system folders like INBOX)
2. Calls ImapClient method
3. Updates SQLite cache (add/rename/remove folder, cascade messages)
4. Updates Tantivy index (remove indexed messages for deleted folders)
5. Returns updated folder list

**Validation rules:**
- Cannot rename/delete system folders: INBOX, Sent, Drafts, Trash, Junk
- Folder name max length: 255 chars
- No special characters that would break IMAP: `{`, `}`, `*`, `%`

**Tests:**
- Create folder → 201 + folder in list
- Rename folder → 200 + old name gone, new name exists
- Delete empty folder → 200 + folder removed
- Delete non-empty folder with force → 200
- Cannot delete INBOX → 400
- Cannot rename to existing name → 409

**Commit:** `feat(api): add folder management endpoints (create, rename, delete, subscribe)`

### Task 2.3: Frontend folder management UI

**Create:** `frontend/src/components/mail/FolderContextMenu.tsx`
**Create:** `frontend/src/components/mail/CreateFolderDialog.tsx`
**Modify:** `frontend/src/components/mail/FolderTree.tsx`
**Modify:** `frontend/src/hooks/useFolders.ts` (add mutation hooks)

**Mutation hooks (in useFolders.ts):**
```typescript
export function useCreateFolder() { ... }
export function useRenameFolder() { ... }
export function useDeleteFolder() { ... }
```

**FolderContextMenu:**
- Right-click on a folder shows context menu:
  - "Rename" (disabled for system folders)
  - "Delete" (disabled for system folders, shows confirmation dialog)
  - "New subfolder"
  - "Mark all as read"
- Positioned at cursor with Radix ContextMenu

**CreateFolderDialog:**
- Radix Dialog with folder name input
- Optional parent folder dropdown
- Validates name length and forbidden characters
- Submit creates folder and refreshes folder list

**FolderTree modifications:**
- Right-click context menu integration
- "New folder" button at bottom of folder list (+ icon)
- Inline rename: double-click folder name → editable input → Enter to save, Escape to cancel
- Drag-and-drop messages onto folders to move them

**Commit:** `feat(frontend): add folder management UI with context menu, create/rename/delete`

---

## Phase 3: Contacts & Address Book

### Task 3.1: V007 migration — contacts table

**Create:** `backend/migrations/V007__contacts.sql`
**Modify:** `backend/src/db/pool.rs` — add migration to array

```sql
CREATE TABLE IF NOT EXISTS contacts (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    company TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    is_favorite INTEGER NOT NULL DEFAULT 0,
    last_contacted TEXT,
    contact_count INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'manual', -- 'manual', 'auto', 'import'
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_contacts_email ON contacts(email);
CREATE INDEX IF NOT EXISTS idx_contacts_name ON contacts(name);
CREATE INDEX IF NOT EXISTS idx_contacts_last_contacted ON contacts(last_contacted DESC);
CREATE INDEX IF NOT EXISTS idx_contacts_contact_count ON contacts(contact_count DESC);
```

**Commit:** `feat(db): add V007 migration for contacts table`

### Task 3.2: Create `db/contacts.rs` module

**Create:** `backend/src/db/contacts.rs`
**Modify:** `backend/src/db/mod.rs`

```rust
pub struct Contact {
    pub id: String,
    pub email: String,
    pub name: String,
    pub company: String,
    pub notes: String,
    pub is_favorite: bool,
    pub last_contacted: Option<String>,
    pub contact_count: i64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

// Functions:
pub fn upsert_contact(conn, contact) -> Result<(), String>
pub fn get_contact(conn, id) -> Result<Option<Contact>, String>
pub fn get_contact_by_email(conn, email) -> Result<Option<Contact>, String>
pub fn list_contacts(conn, search: Option<&str>, limit: u32, offset: u32) -> Result<Vec<Contact>, String>
pub fn delete_contact(conn, id) -> Result<bool, String>
pub fn search_contacts(conn, query: &str, limit: u32) -> Result<Vec<Contact>, String>
pub fn increment_contact_count(conn, email: &str) -> Result<(), String>
pub fn auto_add_contact(conn, email: &str, name: &str) -> Result<(), String>
```

`search_contacts` searches by both name and email with `LIKE '%query%'` for auto-complete.
`auto_add_contact` inserts with `source='auto'` if not already exists.
`increment_contact_count` bumps count + updates `last_contacted`.

**Tests:** Standard CRUD tests.

**Commit:** `feat(db): add contacts CRUD module`

### Task 3.3: Contact API endpoints

**Create:** `backend/src/routes/contacts.rs`
**Modify:** `backend/src/routes/mod.rs`

Routes:
- `GET /api/contacts?q=searchterm&limit=50&offset=0` — list/search contacts
- `POST /api/contacts` — create/update contact
- `GET /api/contacts/{id}` — get single contact
- `DELETE /api/contacts/{id}` — delete contact
- `GET /api/contacts/autocomplete?q=al&limit=10` — fast autocomplete for compose

The autocomplete endpoint combines:
1. Contacts matching the query (by name or email)
2. Recently-sent-to addresses from the messages table (from Sent folder)
3. Deduplicates and ranks by contact_count + recency

**Modify:** `backend/src/routes/send.rs`
After successful send, call `db::contacts::increment_contact_count` for each recipient and `auto_add_contact` for any new addresses.

**Tests:**
- Contact CRUD operations
- Autocomplete returns contacts + recent recipients
- Send auto-adds new contacts

**Commit:** `feat(api): add contacts API with autocomplete`

### Task 3.4: Frontend contacts panel

**Create:** `frontend/src/types/contact.ts`
**Create:** `frontend/src/hooks/useContacts.ts`
**Create:** `frontend/src/components/contacts/ContactsPanel.tsx`
**Create:** `frontend/src/components/contacts/ContactCard.tsx`
**Create:** `frontend/src/components/contacts/ContactDialog.tsx`
**Modify:** `frontend/src/components/shared/NavRail.tsx` (wire Contacts button)
**Modify:** `frontend/src/stores/useUiStore.ts` (add view mode state)

**Contact type:**
```typescript
export interface Contact {
  id: string;
  email: string;
  name: string;
  company: string;
  notes: string;
  is_favorite: boolean;
  last_contacted: string | null;
  contact_count: number;
  source: "manual" | "auto" | "import";
}
```

**useContacts hooks:**
```typescript
export function useContacts(search?: string) { ... }
export function useContact(id: string) { ... }
export function useCreateContact() { ... }
export function useDeleteContact() { ... }
export function useAutocomplete(query: string) { ... }
```

**UI store additions:**
```typescript
viewMode: "mail" | "contacts" | "settings";
setViewMode: (mode: string) => void;
```

**ContactsPanel:**
- Replaces the three-panel mail view when `viewMode === "contacts"`
- Search bar at top for filtering contacts
- Scrollable list of contacts with avatar (initials), name, email, company
- Click to view/edit contact details
- "New contact" button
- Favorite toggle (star icon)
- Alphabetical grouping headers (A, B, C...)

**ContactCard:**
- Contact detail view showing all fields
- Edit/delete buttons
- "Compose email" button (opens compose dialog with pre-filled to)
- Shows recent emails with this contact (query messages by from/to address)

**ContactDialog:**
- Create/edit contact form
- Fields: name, email, company, notes
- Validation: email required, valid format

**NavRail changes:**
- Remove `disabled` from Contacts button
- Click sets `viewMode` to `"contacts"`
- Active indicator when in contacts view

**Commit:** `feat(frontend): add contacts panel with search, CRUD, and favorites`

### Task 3.5: Autocomplete in compose dialog

**Modify:** `frontend/src/components/mail/ComposeDialog.tsx`

Replace plain text inputs for To/CC/BCC with autocomplete inputs:
- As user types, query `GET /api/contacts/autocomplete?q=...` with debounce (200ms)
- Show dropdown with matching contacts (name + email)
- Click or Enter to select → adds as a tag/chip
- Multiple recipients shown as removable chips
- Keyboard navigation: arrow up/down to select, Enter to add, Backspace to remove last chip
- Still supports comma-separated paste of multiple addresses

Use a shared `RecipientInput` component for To, CC, and BCC fields.

**Create:** `frontend/src/components/mail/RecipientInput.tsx`

**Commit:** `feat(frontend): add contact autocomplete to compose dialog`

---

## Phase 4: Enhanced Thread View

### Task 4.1: Improve thread detection in backend

**Modify:** `backend/src/db/messages.rs`

Enhance `get_thread_messages` to use JWZ-style threading:

```rust
/// Build a full thread by walking the References chain.
///
/// 1. Start with the selected message's message_id
/// 2. Find all messages that reference it (in_reply_to or references contains it)
/// 3. Walk up: find messages that this message references
/// 4. Walk down: find all messages that reference any message in the thread
/// 5. Return the full thread sorted chronologically
pub fn get_full_thread(
    conn: &Connection,
    message_id: &str,
    references: Option<&str>,
) -> Result<Vec<ThreadMessage>, String>
```

New struct for thread responses:
```rust
pub struct ThreadMessage {
    pub uid: u32,
    pub folder: String,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub subject: String,
    pub from_address: String,
    pub from_name: String,
    pub to_addresses: String,
    pub date: String,
    pub flags: String,
    pub snippet: String,
    pub body_html: Option<String>,
    pub body_text: Option<String>,
    pub body_cached: bool,
}
```

**Commit:** `feat(db): improve thread detection with full References chain walking`

### Task 4.2: Thread API enhancements

**Modify:** `backend/src/routes/messages.rs`

Update the `get_message` handler to return richer thread data:
- Include `body_html` and `body_text` for cached thread messages (so frontend doesn't need separate fetches)
- Add `thread_count` to the message list response (number of messages in thread)
- Add `GET /api/messages/{folder}/{uid}/thread` endpoint that returns just the thread for lazy loading

**Commit:** `feat(api): enhance thread API with cached bodies and thread count`

### Task 4.3: Frontend conversation thread view

**Modify:** `frontend/src/components/mail/ThreadView.tsx`
**Modify:** `frontend/src/components/mail/ReadingPane.tsx`
**Modify:** `frontend/src/components/mail/MessageListItem.tsx`

**ThreadView enhancements:**
- Show thread as a vertical conversation, not just a list of headers
- Each message in thread: collapsible card with sender avatar (initials), name, date, and body
- Most recent message expanded by default, older messages collapsed
- Click to expand/collapse
- Reply button inline per message (opens compose with correct threading headers)
- Thread message count in header: "Conversation (5 messages)"
- Smooth scroll to newly expanded message

**MessageListItem enhancements:**
- Show thread count badge if `thread_count > 1` (e.g., "[3]" next to subject)
- When thread is collapsed in list, show most recent sender names

**ReadingPane enhancements:**
- When viewing a threaded message, show ThreadView instead of single message
- "Expand all" / "Collapse all" buttons

**Commit:** `feat(frontend): enhance thread view with expandable conversation cards`

---

## Phase 5: Drag-and-Drop Organization

### Task 5.1: Drag messages between folders

**Modify:** `frontend/src/components/mail/MessageListItem.tsx`
**Modify:** `frontend/src/components/mail/FolderTree.tsx`

Use HTML5 Drag and Drop API:

**MessageListItem:**
- `draggable="true"` attribute
- `onDragStart` sets drag data: `{ uid, folder, subject }`
- Visual feedback: ghost image with subject text
- Drag handle icon on hover

**FolderTree:**
- Each folder accepts drops (`onDragOver`, `onDrop`)
- Visual feedback: highlight drop target folder
- On drop: call `useMoveMessage` mutation with target folder
- Show toast: "Moved to {folder}"
- Prevent drop on source folder (no-op)

**Commit:** `feat(frontend): add drag-and-drop message movement between folders`

### Task 5.2: Bulk actions

**Modify:** `frontend/src/components/mail/MessageList.tsx`
**Modify:** `frontend/src/components/mail/MessageListItem.tsx`
**Create:** `frontend/src/components/mail/BulkActionBar.tsx`
**Modify:** `frontend/src/stores/useUiStore.ts`

**UI store additions:**
```typescript
selectedMessageUids: Set<number>;
bulkSelectMode: boolean;
toggleBulkSelect: (uid: number) => void;
selectAll: (uids: number[]) => void;
clearSelection: () => void;
setBulkSelectMode: (mode: boolean) => void;
```

**MessageListItem changes:**
- Checkbox visible on hover or when bulk select mode is active
- Shift+click for range selection
- Cmd/Ctrl+click for multi-select

**BulkActionBar:**
- Appears above message list when 2+ messages selected
- Actions: Mark read, Mark unread, Star, Delete, Move to folder
- "Select all" / "Deselect all" buttons
- Shows count: "3 selected"

**Backend:**
- No changes needed — frontend sends individual mutations per message. For performance, batch them with `Promise.allSettled()` and invalidate queries once at the end.

**Commit:** `feat(frontend): add bulk selection and batch actions for messages`

---

## Phase 6: Final Integration & Verification

### Task 6.1: Keyboard shortcuts

**Create:** `frontend/src/hooks/useKeyboardShortcuts.ts`
**Modify:** `frontend/src/app/(auth)/mail/page.tsx`

Global keyboard shortcuts (when not in an input/textarea):
- `Cmd/Ctrl+K` — focus search bar
- `Cmd/Ctrl+N` — new compose
- `Cmd/Ctrl+Shift+N` — new folder
- `Delete` / `Backspace` — delete selected message
- `E` — archive (move to Archive folder)
- `R` — reply
- `Shift+R` — reply all
- `F` — forward
- `S` — star/unstar
- `U` — mark unread
- `J` / `K` or `↓` / `↑` — next/previous message
- `Enter` — open selected message
- `Escape` — close reading pane / clear search / close dialog

**Commit:** `feat(frontend): add keyboard shortcuts for common actions`

### Task 6.2: Run all tests and build

**Backend:**
```bash
cd backend && cargo test -- -v
cd backend && cargo clippy -- -D warnings
```

**Frontend:**
```bash
cd frontend && pnpm lint
cd frontend && pnpm build
```

**Docker:**
```bash
docker compose up --build -d
```

Fix any failing tests, lint errors, or build issues.

**Commit:** `feat: Epic 5 integration — search, organization, contacts, and threads complete`

---

## Critical Files Reference

| Purpose | File |
|---------|------|
| IMAP trait (extend for folder CRUD) | `backend/src/imap/client.rs` |
| Router registration | `backend/src/routes/mod.rs` |
| Handler pattern reference | `backend/src/routes/messages.rs` |
| DB pattern reference | `backend/src/db/messages.rs` |
| Migration runner | `backend/src/db/pool.rs` |
| Data dir (Tantivy index location) | `backend/src/config.rs` |
| Send handler (auto-add contacts) | `backend/src/routes/send.rs` |
| Compose dialog (add autocomplete) | `frontend/src/components/mail/ComposeDialog.tsx` |
| Thread view (enhance) | `frontend/src/components/mail/ThreadView.tsx` |
| Message list item (drag + bulk) | `frontend/src/components/mail/MessageListItem.tsx` |
| Folder tree (context menu + drop) | `frontend/src/components/mail/FolderTree.tsx` |
| UI store (search + bulk + view mode) | `frontend/src/stores/useUiStore.ts` |
| Nav rail (wire contacts) | `frontend/src/components/shared/NavRail.tsx` |
| Three-panel layout | `frontend/src/components/shared/ThreePanelLayout.tsx` |

---

## Summary

| Phase | Task | Description | Backend | Frontend |
|-------|------|-------------|---------|----------|
| 1 | 1.1 | Add Tantivy dependency | X | |
| 1 | 1.2 | Search engine module | X | |
| 1 | 1.3 | Integrate indexing into sync | X | |
| 1 | 1.4 | Search API endpoint | X | |
| 1 | 1.5 | Search bar + results UI | | X |
| 2 | 2.1 | IMAP folder CRUD methods | X | |
| 2 | 2.2 | Folder management API | X | |
| 2 | 2.3 | Folder management UI | | X |
| 3 | 3.1 | Contacts migration (V007) | X | |
| 3 | 3.2 | Contacts DB module | X | |
| 3 | 3.3 | Contacts API + autocomplete | X | |
| 3 | 3.4 | Contacts panel UI | | X |
| 3 | 3.5 | Autocomplete in compose | | X |
| 4 | 4.1 | Improved thread detection | X | |
| 4 | 4.2 | Thread API enhancements | X | |
| 4 | 4.3 | Conversation thread view | | X |
| 5 | 5.1 | Drag-and-drop messages | | X |
| 5 | 5.2 | Bulk actions | | X |
| 6 | 6.1 | Keyboard shortcuts | | X |
| 6 | 6.2 | Final integration + tests | X | X |

**Parallelizable tracks:**
- Phases 1-4 are independent and can be worked in any order
- Within Phase 3, tasks 3.1-3.3 (backend) must precede 3.4-3.5 (frontend)
- Phase 5 depends on existing message list and folder tree (already built)
- Phase 6 should be done last

## Verification

1. **Backend tests:** `cargo test --manifest-path backend/Cargo.toml` — all search, folder mgmt, contacts, and thread tests pass
2. **Frontend build:** `cd frontend && pnpm build` — no type errors
3. **Search E2E:** Type query → results appear → click result → message opens in correct folder
4. **Folder mgmt E2E:** Create folder → rename it → move message into it → delete it
5. **Contacts E2E:** Add contact → compose email → autocomplete shows contact → send → contact count increments
6. **Thread E2E:** Open threaded message → conversation view shows all messages → expand/collapse works
7. **Drag E2E:** Drag message from Inbox → drop on Sent folder → message moves
8. **Docker:** `docker build -t oxi-email:epic5 .` — image builds successfully
