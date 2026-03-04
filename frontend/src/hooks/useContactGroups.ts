"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPost, apiPut, apiDelete } from "@/lib/api";
import type { ContactGroup, Contact } from "@/types/contact";

interface ListGroupsResponse {
  groups: ContactGroup[];
}

interface GroupMembersResponse {
  members: Contact[];
}

export function useContactGroups() {
  return useQuery({
    queryKey: ["contact-groups"],
    queryFn: () => apiGet<ListGroupsResponse>("/contact-groups"),
  });
}

export function useCreateGroup() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string) =>
      apiPost<{ id: string; name: string }>("/contact-groups", { name }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["contact-groups"] });
    },
  });
}

export function useUpdateGroup() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      apiPut<{ id: string; name: string }>(`/contact-groups/${id}`, { name }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["contact-groups"] });
    },
  });
}

export function useDeleteGroup() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiDelete(`/contact-groups/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["contact-groups"] });
    },
  });
}

export function useGroupMembers(groupId: string | null) {
  return useQuery({
    queryKey: ["contact-group-members", groupId],
    queryFn: () =>
      apiGet<GroupMembersResponse>(`/contact-groups/${groupId}/members`),
    enabled: !!groupId,
  });
}

export function useAddGroupMember() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      groupId,
      contactId,
    }: {
      groupId: string;
      contactId: string;
    }) =>
      apiPost(`/contact-groups/${groupId}/members`, {
        contact_id: contactId,
      }),
    onSuccess: (_, { groupId }) => {
      queryClient.invalidateQueries({
        queryKey: ["contact-group-members", groupId],
      });
      queryClient.invalidateQueries({ queryKey: ["contact-groups"] });
    },
  });
}

export function useRemoveGroupMember() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      groupId,
      contactId,
    }: {
      groupId: string;
      contactId: string;
    }) => apiDelete(`/contact-groups/${groupId}/members/${contactId}`),
    onSuccess: (_, { groupId }) => {
      queryClient.invalidateQueries({
        queryKey: ["contact-group-members", groupId],
      });
      queryClient.invalidateQueries({ queryKey: ["contact-groups"] });
    },
  });
}
