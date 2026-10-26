"use client";

import { useEffect, useRef } from "react";
import { toast } from "sonner";
import {
  useQuery,
  useInfiniteQuery,
  useMutation,
  useMutationState,
  useQueryClient,
} from "@tanstack/react-query";
import type { InfiniteData, QueryClient } from "@tanstack/react-query";
import { apiGet, apiPatch, apiPost, apiDelete } from "@/lib/api";
import { useWsStatus } from "@/lib/ws-context";
import { useUiStore } from "@/stores/useUiStore";
import { useAuthStore } from "@/stores/useAuthStore";
import type { MessagesResponse, MessageDetail, SearchResponse } from "@/types/message";

const PER_PAGE = 50;

const searchReconciliationTimers = new WeakMap<QueryClient, ReturnType<typeof setTimeout>>();

function reconcileSearchAfterMoves(queryClient: QueryClient, operation: MoveMessageOperation) {
  clearTimeout(searchReconciliationTimers.get(queryClient));
  // onSettled still counts as pending. Check after its state transition, and
  // coalesce simultaneous settlements rather than letting both skip refresh.
  searchReconciliationTimers.set(queryClient, setTimeout(() => {
    searchReconciliationTimers.delete(queryClient);
    if (isCurrentMove(operation)
      && queryClient.getMutationCache().getAll().some(move => move.state.variables === operation)
      && queryClient.isMutating({ mutationKey: ["move-message"] }) === 0) {
      void queryClient.invalidateQueries({ queryKey: ["search"] });
    }
  }, 0));
}

type MoveMessageVariables = { fromFolder: string; toFolder: string; uid: number };
type MessageRemovalVariables = { fromFolder: string; toFolder?: string; uid: number };
type MoveMessageOperation = MessageRemovalVariables & {
  accountId: string | null;
  cancelled: boolean;
  ready: Promise<void>;
  cancellation: Promise<void>;
  cancel: () => void;
  release: () => void;
};

const moveQueueTails = new WeakMap<QueryClient, Promise<void>>();

function createMoveOperation(queryClient: QueryClient, queued: boolean, variables: MessageRemovalVariables): MoveMessageOperation {
  let cancel!: () => void;
  let release!: () => void;
  const cancellation = new Promise<void>(resolve => { cancel = resolve; });
  const done = new Promise<void>(resolve => { release = resolve; });
  const ready = queued ? moveQueueTails.get(queryClient) ?? Promise.resolve() : Promise.resolve();
  if (queued) moveQueueTails.set(queryClient, done);
  return {
    ...variables,
    accountId: useAuthStore.getState().activeAccountId,
    cancelled: false,
    ready,
    cancellation,
    cancel,
    release: () => {
      release();
      if (moveQueueTails.get(queryClient) === done) moveQueueTails.delete(queryClient);
    },
  };
}

function isCurrentMove(operation: MoveMessageOperation) {
  return !operation.cancelled && operation.accountId === useAuthStore.getState().activeAccountId;
}

// Also used for the initial cache update. Count actual removals once across
// pages: every page carries the same folder-wide total_count.
export function hideMovedMessages(
  data: InfiniteData<MessagesResponse>,
  folder: string,
  uids: ReadonlySet<number>,
): InfiniteData<MessagesResponse> {
  const removed = new Set(data.pages.flatMap(page => page.messages
    .filter(message => message.folder === folder && uids.has(message.uid))
    .map(message => message.uid)));
  if (!removed.size) return data;
  return {
    ...data,
    pages: data.pages.map(page => ({
      ...page,
      messages: page.messages.filter(message => !(message.folder === folder && removed.has(message.uid))),
      total_count: Math.max(0, page.total_count - removed.size),
    })),
  };
}

export function useMessages(folder: string) {
  const { status } = useWsStatus();
  const pendingMoves = useMutationState<MoveMessageOperation>({
    filters: { mutationKey: ["move-message"], status: "pending" },
    select: mutation => mutation.state.variables as MoveMessageOperation,
  });
  const query = useInfiniteQuery({
    queryKey: ["messages", folder],
    queryFn: ({ pageParam = 0 }) =>
      apiGet<MessagesResponse>(
        `/folders/${encodeURIComponent(folder)}/messages?page=${pageParam}&per_page=${PER_PAGE}`,
      ),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => {
      const fetched = (lastPage.page + 1) * lastPage.per_page;
      return fetched < lastPage.total_count ? lastPage.page + 1 : undefined;
    },
    enabled: !!folder,
    refetchInterval: status === "connected" ? false : 60_000,
  });
  // Read-time overlay survives any subsequent flag/WS/polling refetch, and
  // sees moves started by other components sharing this QueryClient.
  const hidden = new Set(pendingMoves.filter(move => isCurrentMove(move) && move.fromFolder === folder).map(move => move.uid));
  return { ...query, data: query.data ? hideMovedMessages(query.data, folder, hidden) : query.data };
}

export function useMessage(folder: string, uid: number) {
  return useQuery({
    queryKey: ["message", folder, uid],
    queryFn: () =>
      apiGet<MessageDetail>(
        `/messages/${encodeURIComponent(folder)}/${uid}`,
      ),
    enabled: !!folder && uid > 0,
    staleTime: 60_000,
    retry: (failureCount, error) => {
      // Don't retry "not found" — the message was deleted from IMAP.
      if (error instanceof Error && error.message.includes("not found")) return false;
      return failureCount < 2;
    },
  });
}

export function useUpdateFlags() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      folder,
      uid,
      flags,
      add,
    }: {
      folder: string;
      uid: number;
      flags: string[];
      add: boolean;
    }) =>
      apiPatch(`/messages/${encodeURIComponent(folder)}/${uid}/flags`, {
        flags,
        add,
      }),
    onSuccess: (_, { folder, uid }) => {
      queryClient.invalidateQueries({ queryKey: ["messages", folder] });
      queryClient.invalidateQueries({ queryKey: ["message", folder, uid] });
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

// Moves and permanent deletes share removal, rollback and reconciliation. The
// mutation key covers both so one action cannot resurrect a pending sibling.
function useRemoveMessage({ queued = false }: { queued?: boolean } = {}) {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationKey: ["move-message"],
    // The local queue includes reconciliation, but cancellation must settle
    // even while offline or unfocused (TanStack paused scopes cannot do that).
    networkMode: "always",
    mutationFn: async (operation: MoveMessageOperation) => {
      await Promise.race([operation.ready, operation.cancellation]);
      if (!isCurrentMove(operation)) throw new Error("Move cancelled because the account changed");
      if (!operation.toFolder) {
        return apiDelete(`/messages/${encodeURIComponent(operation.fromFolder)}/${operation.uid}`);
      }
      return apiPost("/messages/move", {
        from_folder: operation.fromFolder,
        to_folder: operation.toFolder,
        uid: operation.uid,
      });
    },
    onMutate: async (operation) => {
      const { fromFolder, toFolder, uid } = operation;
      const ownMutation = queryClient.getMutationCache().getAll()
        .find(move => move.state.variables === operation);
      // Release cleared work independently of network/focus state. Requests
      // already sent may finish, but their callbacks must not touch new data.
      const unsubscribe = queryClient.getMutationCache().subscribe(event => {
        if (event.type !== "removed" || event.mutation !== ownMutation) return;
        operation.cancelled = true;
        unsubscribe();
        operation.cancel();
        operation.release();
      });
      if (!isCurrentMove(operation)) return { unsubscribe };
      // Auto-advance: if the moved message is selected, select the next (or previous) message.
      const { selectedMessageUid, selectMessage } = useUiStore.getState();
      if (useUiStore.getState().activeFolder === fromFolder && selectedMessageUid === uid) {
        const prev = queryClient.getQueryData<InfiniteData<MessagesResponse>>(["messages", fromFolder]);
        if (prev) {
          const allMessages = prev.pages.flatMap((p) => p.messages);
          const idx = allMessages.findIndex((m) => m.uid === uid);
          const pendingUids = new Set(queryClient.getMutationCache()
            .findAll({ mutationKey: ["move-message"], status: "pending" })
            .map(move => move.state.variables as MoveMessageVariables)
            .filter(move => move.fromFolder === fromFolder)
            .map(move => move.uid));
          const available = (message: MessagesResponse["messages"][number]) => !pendingUids.has(message.uid);
          const nextMsg = allMessages.slice(idx + 1).find(available)
            ?? allMessages.slice(0, Math.max(0, idx)).reverse().find(available);
          selectMessage(nextMsg?.uid ?? null);
        } else {
          selectMessage(null);
        }
      }

      // Cancel in-flight fetches so they don't overwrite our optimistic update.
      await Promise.all([
        queryClient.cancelQueries({ queryKey: ["messages", fromFolder] }),
        ...(toFolder ? [queryClient.cancelQueries({ queryKey: ["messages", toFolder] })] : []),
        queryClient.cancelQueries({ queryKey: ["search"] })
      ]);

      if (!isCurrentMove(operation)) return { unsubscribe };
      const searchSnapshots = queryClient.getQueriesData<SearchResponse>({ queryKey: ["search"] });
      for (const [key, search] of searchSnapshots) {
        if (!search) continue;
        const results = search.results.filter(item => !(item.folder === fromFolder && item.uid === uid));
        const removed = search.results.length - results.length;
        if (removed) queryClient.setQueryData(key, { ...search, results, total_count: Math.max(0, search.total_count - removed) });
      }

      const prevFrom = queryClient.getQueryData<InfiniteData<MessagesResponse>>(
        ["messages", fromFolder],
      );
      // Remove from source folder cache.
      if (prevFrom) {
        queryClient.setQueryData<InfiniteData<MessagesResponse>>(
          ["messages", fromFolder],
          hideMovedMessages(prevFrom, fromFolder, new Set([uid])),
        );
      }

      // IMAP UIDs are folder-local. Only a destination refetch can supply
      // the moved message's new UID; never insert a source UID here.

      return { prevFrom, searchSnapshots, selectedMessageUid, advancedSelection: useUiStore.getState().selectedMessageUid, unsubscribe };
    },
    onError: (err, operation, context) => {
      if (!isCurrentMove(operation)) {
        toast.error("Pending move cancelled because the account changed");
        return;
      }
      const { fromFolder, uid } = operation;
      // Restore only this move's result, not a whole snapshot that could
      // resurrect other messages being moved concurrently.
      for (const [key, snapshot] of context?.searchSnapshots ?? []) {
        const item = snapshot?.results.find(result => result.folder === fromFolder && result.uid === uid);
        if (!item) continue;
        queryClient.setQueryData<SearchResponse>(key, current => {
          if (!current || current.results.some(result => result.folder === fromFolder && result.uid === uid)) return current;
          const results = [...current.results];
          const index = snapshot!.results.indexOf(item);
          results.splice(Math.min(index, results.length), 0, item);
          return { ...current, results, total_count: current.total_count + 1 };
        });
      }
      toast.error(err instanceof Error ? err.message : "Failed to remove message");
      const ui = useUiStore.getState();
      if (context && ui.activeFolder === fromFolder && ui.selectedMessageUid === context.advancedSelection) {
        ui.selectMessage(context.selectedMessageUid);
      }
      // Restore only this row, never a snapshot containing another move's
      // removed rows or stale flags. Destination was never modified by us.
      const snapshot = context?.prevFrom;
      const originalPage = snapshot?.pages.find(page => page.messages.some(message => message.uid === uid && message.folder === fromFolder));
      const item = originalPage?.messages.find(message => message.uid === uid && message.folder === fromFolder);
      if (originalPage && item) {
        queryClient.setQueryData<InfiniteData<MessagesResponse>>(["messages", fromFolder], current => {
          if (!current || current.pages.some(page => page.messages.some(message => message.uid === uid && message.folder === fromFolder))) return current;
          const targetPage = current.pages.findIndex(page => page.page === originalPage.page);
          // If that page was evicted, let reconciliation fetch it rather than
          // inserting into an unrelated page or recreating an obsolete cache.
          if (targetPage < 0) return current;
          return { ...current, pages: current.pages.map((page, index) => {
            const messages = [...page.messages];
            if (index === targetPage) messages.splice(Math.min(originalPage.messages.indexOf(item), messages.length), 0, item);
            return { ...page, messages, total_count: page.total_count + 1 };
          }) };
        });
      }
    },
    onSettled: async (_data, error, operation, context) => {
      try {
        if (!isCurrentMove(operation)) return;
        const { fromFolder, toFolder, uid } = operation;
        // Keep the overlay through reconciliation and replace pre-move fetches.
        // TanStack's silent refetch cancellation chains waiting callers onto
        // the replacement fetch, so overlapping settlements stay protected.
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: ["messages", fromFolder] }),
          ...(toFolder ? [queryClient.invalidateQueries({ queryKey: ["messages", toFolder] })] : []),
          queryClient.invalidateQueries({ queryKey: ["folders"] }),
        ]);
        if (!isCurrentMove(operation)) return;
        // A failed list refresh retains old cache data. A confirmed successful
        // move must not reappear when its pending overlay is released.
        if (!error) {
          queryClient.setQueryData<InfiniteData<MessagesResponse>>(["messages", fromFolder], current =>
            current ? hideMovedMessages(current, fromFolder, new Set([uid])) : current);
        }
        reconcileSearchAfterMoves(queryClient, operation);
      } finally {
        context?.unsubscribe();
        operation.release();
      }
    },
  });
  return {
    ...mutation,
    mutate: (variables: MessageRemovalVariables, options?: Parameters<typeof mutation.mutate>[1]) => {
      // Synchronous cache lookup also catches double-clicks before rerender.
      if (queued && queryClient.getMutationCache().findAll({ mutationKey: ["move-message"], status: "pending" })
        .some(move => {
          const pending = move.state.variables as MoveMessageVariables;
          return pending.fromFolder === variables.fromFolder && pending.uid === variables.uid;
        })) return;
      mutation.mutate(createMoveOperation(queryClient, queued, variables), options);
    },
    mutateAsync: (variables: MessageRemovalVariables, options?: Parameters<typeof mutation.mutateAsync>[1]) =>
      mutation.mutateAsync(createMoveOperation(queryClient, queued, variables), options),
  };
}

export function useMoveMessage(options: { queued?: boolean } = {}) {
  const mutation = useRemoveMessage(options);
  return {
    ...mutation,
    mutate: (variables: MoveMessageVariables, options?: Parameters<typeof mutation.mutate>[1]) =>
      mutation.mutate(variables, options),
    mutateAsync: (variables: MoveMessageVariables, options?: Parameters<typeof mutation.mutateAsync>[1]) =>
      mutation.mutateAsync(variables, options),
  };
}

export function useDeleteMessage() {
  const mutation = useRemoveMessage();
  type DeleteMessageVariables = { folder: string; uid: number };
  return {
    ...mutation,
    mutate: ({ folder, uid }: DeleteMessageVariables, options?: Parameters<typeof mutation.mutate>[1]) =>
      mutation.mutate({ fromFolder: folder, uid }, options),
    mutateAsync: ({ folder, uid }: DeleteMessageVariables, options?: Parameters<typeof mutation.mutateAsync>[1]) =>
      mutation.mutateAsync({ fromFolder: folder, uid }, options),
  };
}

/**
 * Prefetch the first page of messages for each folder in the background.
 * This triggers the backend to sync messages from IMAP lazily so folder
 * counts are populated and messages are ready when the user clicks a folder.
 */
export function usePrefetchAllFolders(folderNames: string[], activeFolder: string) {
  const queryClient = useQueryClient();
  const prefetched = useRef(false);

  useEffect(() => {
    if (prefetched.current || folderNames.length === 0) return;
    prefetched.current = true;

    // Prefetch each folder except the active one (already loaded by MessageList).
    for (const name of folderNames) {
      if (name === activeFolder) continue;
      queryClient.prefetchInfiniteQuery({
        queryKey: ["messages", name],
        queryFn: () =>
          apiGet<MessagesResponse>(
            `/folders/${encodeURIComponent(name)}/messages?page=0&per_page=${PER_PAGE}`,
          ),
        initialPageParam: 0,
      });
    }

    // Folder counts are updated by WebSocket events and background sync —
    // no need for a timer-based invalidation.
  }, [folderNames, activeFolder, queryClient]);
}
