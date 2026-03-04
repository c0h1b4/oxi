"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";

interface GroupDialogProps {
  open: boolean;
  onClose: () => void;
  onSubmit: (name: string) => void;
  isPending: boolean;
  initialName?: string;
  title?: string;
}

/** Inner form that resets state naturally via remount when `open` toggles. */
function GroupForm({
  onClose,
  onSubmit,
  isPending,
  initialName,
  title,
}: Omit<GroupDialogProps, "open">) {
  const [name, setName] = useState(initialName ?? "");

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="relative z-10 w-full max-w-sm rounded-lg border border-border bg-card p-6 shadow-lg">
        <h2 className="text-lg font-semibold text-foreground">{title}</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (name.trim()) {
              onSubmit(name.trim());
            }
          }}
          className="mt-4 space-y-4"
        >
          <div>
            <label
              htmlFor="group-name"
              className="block text-sm font-medium text-foreground"
            >
              Group name
            </label>
            <input
              id="group-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Work, Family, Friends"
              autoFocus
              className="mt-1 h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none dark:bg-input/30"
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onClose}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={!name.trim() || isPending}
            >
              {isPending ? "Saving..." : "Save"}
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}

export function GroupDialog({
  open,
  ...rest
}: GroupDialogProps) {
  if (!open) return null;
  return <GroupForm {...rest} />;
}
