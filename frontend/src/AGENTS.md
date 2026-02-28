# FRONTEND KNOWLEDGE BASE

**Location:** /frontend/src
**Tech Stack:** Next.js 16, React 19, TanStack Query, Zustand, Tailwind 4, Tiptap, shadcn/ui.

## OVERVIEW
Next.js 16 webmail client using React 19 and Tailwind 4. Connects to Rust backend via JSON API and WebSockets for real-time IMAP updates.

## STRUCTURE
```
src/
├── app/             # App Router routes
│   ├── (auth)/mail/ # Main mail interface
│   └── login/       # Auth entry point
├── components/      # Feature-based UI
│   ├── mail/        # Compose, ReadingPane, MessageList
│   ├── contacts/    # Address book UI
│   ├── settings/    # User preferences
│   ├── shared/      # Layout, Sidebar, SearchBar
│   └── ui/          # shadcn/ui primitives
├── hooks/           # Data fetching + logic
├── stores/          # Zustand client state
├── lib/             # API client, utils, query-client
├── types/           # TypeScript definitions
└── styles/          # Tailwind 4 CSS
```

## WHERE TO LOOK

| Task | Location | Key Files |
| :--- | :--- | :--- |
| Routing | `app/` | `layout.tsx`, `page.tsx` |
| API Client | `lib/api.ts` | `apiGet`, `apiPost`, `apiPatch` |
| Mail Data | `hooks/` | `useMessages.ts`, `useFolders.ts` |
| State | `stores/` | `useAuthStore`, `useComposeStore` |
| Editor | `components/mail/` | `ComposeDialog.tsx` (Tiptap) |
| Real-time | `hooks/` | `useWebSocket.ts` |
| Styling | `styles/` | `globals.css` (Tailwind 4) |

## CONVENTIONS

**Data Fetching**
- Use TanStack Query for all server state.
- Hooks in `hooks/` wrap query/mutation logic.
- API calls throw on error. `lib/api.ts` handles JSON parsing.

**State Management**
- Zustand for ephemeral UI state (sidebar, active compose, auth session).
- Avoid prop drilling. Use stores for cross-component coordination.

**Styling**
- Tailwind 4. Use CSS variables for themes.
- No `tailwind.config.js`. Configuration lives in `globals.css` via `@theme`.
- Use shadcn/ui components for all primitives.

**Components**
- Functional components with TypeScript.
- Use React 19 `use` hook or `Suspense` where appropriate.
- Keep logic in hooks, keep components focused on rendering.

## COMPLEXITY HOTSPOTS

**ComposeDialog.tsx (824 lines)**
- Manages Tiptap editor instance.
- Handles attachment uploads and state.
- Complex validation for recipients and identity selection.

**ReadingPane.tsx (585 lines)**
- HTML sanitization for message bodies.
- Iframe isolation for rendering untrusted content.
- Thread expansion and message action logic.

**useWebSocket.ts**
- Manages connection lifecycle.
- Maps backend events to TanStack Query cache invalidations.
- Handles reconnection logic and heartbeat.
