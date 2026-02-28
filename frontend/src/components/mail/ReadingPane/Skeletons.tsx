export function HeaderSkeleton() {
  return (
    <div className="space-y-3 border-b border-border p-4">
      <div className="h-6 w-3/4 animate-pulse rounded bg-muted" />
      <div className="h-4 w-1/2 animate-pulse rounded bg-muted" />
      <div className="h-4 w-1/3 animate-pulse rounded bg-muted" />
      <div className="h-4 w-1/4 animate-pulse rounded bg-muted" />
    </div>
  );
}

export function BodySkeleton() {
  return (
    <div className="space-y-2 p-4">
      <div className="h-4 w-full animate-pulse rounded bg-muted" />
      <div className="h-4 w-full animate-pulse rounded bg-muted" />
      <div className="h-4 w-5/6 animate-pulse rounded bg-muted" />
      <div className="h-4 w-full animate-pulse rounded bg-muted" />
      <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
    </div>
  );
}
