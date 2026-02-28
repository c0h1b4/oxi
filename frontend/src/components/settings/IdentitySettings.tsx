"use client";

import { useCallback, useState } from "react";
import { Plus, Pencil, Trash2, Star, Check, Loader2 } from "lucide-react";
import { toast } from "sonner";
import {
  useIdentities,
  useCreateIdentity,
  useUpdateIdentity,
  useDeleteIdentity,
} from "@/hooks/useIdentities";
import type { Identity } from "@/types/identity";
import { cn } from "@/lib/utils";

interface IdentityFormData {
  email: string;
  display_name: string;
  signature_html: string;
  is_default: boolean;
}

const emptyForm: IdentityFormData = {
  email: "",
  display_name: "",
  signature_html: "",
  is_default: false,
};

function IdentityForm({
  initial,
  onSave,
  onCancel,
  saving,
}: {
  initial: IdentityFormData;
  onSave: (data: IdentityFormData) => void;
  onCancel: () => void;
  saving: boolean;
}) {
  const [form, setForm] = useState<IdentityFormData>(initial);

  return (
    <div className="space-y-4 rounded-lg border border-border bg-accent/30 p-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <div>
          <label className="mb-1 block text-xs font-medium text-muted-foreground">
            Email address
          </label>
          <input
            type="email"
            value={form.email}
            onChange={(e) => setForm({ ...form, email: e.target.value })}
            className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus:ring-1 focus:ring-primary"
            placeholder="you@example.com"
          />
        </div>
        <div>
          <label className="mb-1 block text-xs font-medium text-muted-foreground">
            Display name
          </label>
          <input
            type="text"
            value={form.display_name}
            onChange={(e) => setForm({ ...form, display_name: e.target.value })}
            className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus:ring-1 focus:ring-primary"
            placeholder="Your Name"
          />
        </div>
      </div>
      <div>
        <label className="mb-1 block text-xs font-medium text-muted-foreground">
          Signature (HTML)
        </label>
        <textarea
          value={form.signature_html}
          onChange={(e) => setForm({ ...form, signature_html: e.target.value })}
          rows={4}
          className="w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-xs outline-none focus:ring-1 focus:ring-primary"
          placeholder="<p>Best regards,<br>Your Name</p>"
        />
      </div>
      <div className="flex items-center gap-2">
        <input
          type="checkbox"
          id="is-default"
          checked={form.is_default}
          onChange={(e) => setForm({ ...form, is_default: e.target.checked })}
          className="size-4 rounded border-border"
        />
        <label htmlFor="is-default" className="text-sm text-muted-foreground">
          Set as default identity
        </label>
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={() => onSave(form)}
          disabled={saving || !form.email.trim()}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-medium transition-colors",
            "bg-primary text-primary-foreground hover:bg-primary/90",
            "disabled:cursor-not-allowed disabled:opacity-50",
          )}
        >
          {saving ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Check className="size-4" />
          )}
          Save
        </button>
        <button
          onClick={onCancel}
          className="rounded-lg px-4 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}

export function IdentitySettings() {
  const { data: identities, isLoading } = useIdentities();
  const createIdentity = useCreateIdentity();
  const updateIdentity = useUpdateIdentity();
  const deleteIdentity = useDeleteIdentity();

  const [showAddForm, setShowAddForm] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);

  const handleCreate = useCallback(
    (data: IdentityFormData) => {
      createIdentity.mutate(
        {
          email: data.email,
          display_name: data.display_name || undefined,
          signature_html: data.signature_html || undefined,
          is_default: data.is_default || undefined,
        },
        {
          onSuccess: () => {
            toast.success("Identity created");
            setShowAddForm(false);
          },
          onError: (e) => toast.error(`Failed to create: ${e.message}`),
        },
      );
    },
    [createIdentity],
  );

  const handleUpdate = useCallback(
    (id: number, data: IdentityFormData) => {
      updateIdentity.mutate(
        {
          id,
          data: {
            email: data.email,
            display_name: data.display_name,
            signature_html: data.signature_html,
            is_default: data.is_default,
          },
        },
        {
          onSuccess: () => {
            toast.success("Identity updated");
            setEditingId(null);
          },
          onError: (e) => toast.error(`Failed to update: ${e.message}`),
        },
      );
    },
    [updateIdentity],
  );

  const handleDelete = useCallback(
    (identity: Identity) => {
      if (identity.is_default) {
        toast.error("Cannot delete the default identity");
        return;
      }
      deleteIdentity.mutate(identity.id, {
        onSuccess: () => toast.success("Identity deleted"),
        onError: (e) => toast.error(`Failed to delete: ${e.message}`),
      });
    },
    [deleteIdentity],
  );

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        Loading identities...
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-semibold">Sender Identities</h2>
          <p className="mt-0.5 text-sm text-muted-foreground">
            Manage email addresses and signatures used when sending messages.
          </p>
        </div>
        {!showAddForm && (
          <button
            onClick={() => setShowAddForm(true)}
            className="inline-flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-sm font-medium transition-colors hover:bg-accent"
          >
            <Plus className="size-4" />
            Add identity
          </button>
        )}
      </div>

      {showAddForm && (
        <IdentityForm
          initial={emptyForm}
          onSave={handleCreate}
          onCancel={() => setShowAddForm(false)}
          saving={createIdentity.isPending}
        />
      )}

      <div className="space-y-3">
        {identities?.map((identity) =>
          editingId === identity.id ? (
            <IdentityForm
              key={identity.id}
              initial={{
                email: identity.email,
                display_name: identity.display_name,
                signature_html: identity.signature_html,
                is_default: identity.is_default,
              }}
              onSave={(data) => handleUpdate(identity.id, data)}
              onCancel={() => setEditingId(null)}
              saving={updateIdentity.isPending}
            />
          ) : (
            <div
              key={identity.id}
              className="flex items-center justify-between rounded-lg border border-border p-4"
            >
              <div className="flex items-center gap-3">
                {identity.is_default && (
                  <Star className="size-4 shrink-0 fill-primary text-primary" />
                )}
                <div>
                  <div className="text-sm font-medium">
                    {identity.display_name
                      ? `${identity.display_name} <${identity.email}>`
                      : identity.email}
                  </div>
                  {identity.signature_html && (
                    <p className="mt-0.5 text-xs text-muted-foreground">
                      Has signature
                    </p>
                  )}
                </div>
              </div>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => setEditingId(identity.id)}
                  className="rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
                  title="Edit"
                >
                  <Pencil className="size-4" />
                </button>
                <button
                  onClick={() => handleDelete(identity)}
                  disabled={identity.is_default}
                  className="rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-destructive disabled:cursor-not-allowed disabled:opacity-30"
                  title={
                    identity.is_default
                      ? "Cannot delete default identity"
                      : "Delete"
                  }
                >
                  <Trash2 className="size-4" />
                </button>
              </div>
            </div>
          ),
        )}
      </div>
    </div>
  );
}
