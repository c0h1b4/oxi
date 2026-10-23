import { toast } from "sonner";

import { useAuthStore } from "@/stores/useAuthStore";

import { apiCredentials, apiUrl } from "@/lib/api-url";

interface ApiError {
  code?: string;
  message?: string;
  status?: number;
  accountId?: string;
}

interface ErrorResponse {
  error?: ApiError;
}

interface AccountsResponse {
  accounts: Array<{
    id: string;
    email: string;
    imapHost: string;
    smtpHost: string;
  }>;
  browserSessionExpired?: boolean;
}

function handleAccountSessionExpired(error: ApiError): void {
  if (error.code === "ACCOUNT_SESSION_EXPIRED" && error.accountId) {
    useAuthStore.getState().removeAccount(error.accountId);
    toast.error(error.message ?? "Account session expired");
    fetch(apiUrl(`/auth/accounts/${error.accountId}`), {
      method: "DELETE",
      headers: { "X-Requested-With": "XMLHttpRequest" },
      credentials: apiCredentials,
    }).catch(() => {});
  }
}

async function parseErrorResponse(res: Response): Promise<Error> {
  const data: ErrorResponse = await res.json();
  const error = data.error ?? {};
  handleAccountSessionExpired(error);
  return new Error(error.message ?? "An unexpected error occurred");
}

function getActiveAccountHeader(): Record<string, string> {
  const activeId = useAuthStore.getState().activeAccountId;
  return activeId ? { "X-Active-Account": activeId } : {};
}

export async function fetchAccounts(): Promise<AccountsResponse> {
  const res = await fetch(apiUrl("/auth/accounts"), {
    credentials: apiCredentials,
  });

  if (!res.ok) {
    throw await parseErrorResponse(res);
  }

  const data: AccountsResponse = await res.json();
  
  if (data.browserSessionExpired) {
    useAuthStore.getState().setAccounts([]);
  }
  
  return data;
}

export async function apiPost<T>(
  path: string,
  body: Record<string, unknown>,
): Promise<T> {
  const res = await fetch(apiUrl(path), {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Requested-With": "XMLHttpRequest",
      ...getActiveAccountHeader(),
    },
    credentials: apiCredentials,
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    throw await parseErrorResponse(res);
  }

  return res.json();
}

export async function apiGet<T>(path: string): Promise<T> {
  const res = await fetch(apiUrl(path), {
    headers: {
      ...getActiveAccountHeader(),
    },
    credentials: apiCredentials,
  });

  if (!res.ok) {
    throw await parseErrorResponse(res);
  }

  return res.json();
}

export async function apiPatch<T>(
  path: string,
  body: Record<string, unknown>,
): Promise<T> {
  const res = await fetch(apiUrl(path), {
    method: "PATCH",
    headers: {
      "Content-Type": "application/json",
      "X-Requested-With": "XMLHttpRequest",
      ...getActiveAccountHeader(),
    },
    credentials: apiCredentials,
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    throw await parseErrorResponse(res);
  }

  return res.json();
}

export async function apiPostFormData<T>(
  path: string,
  formData: FormData,
): Promise<T> {
  const res = await fetch(apiUrl(path), {
    method: "POST",
    headers: {
      "X-Requested-With": "XMLHttpRequest",
      ...getActiveAccountHeader(),
    },
    credentials: apiCredentials,
    body: formData,
  });

  if (!res.ok) {
    throw await parseErrorResponse(res);
  }

  return res.json();
}

export async function apiPut<T>(
  path: string,
  body: Record<string, unknown>,
): Promise<T> {
  const res = await fetch(apiUrl(path), {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
      "X-Requested-With": "XMLHttpRequest",
      ...getActiveAccountHeader(),
    },
    credentials: apiCredentials,
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    throw await parseErrorResponse(res);
  }

  return res.json();
}

export async function apiDelete<T>(path: string): Promise<T> {
  const res = await fetch(apiUrl(path), {
    method: "DELETE",
    headers: {
      "X-Requested-With": "XMLHttpRequest",
      ...getActiveAccountHeader(),
    },
    credentials: apiCredentials,
  });

  if (!res.ok) {
    throw await parseErrorResponse(res);
  }

  return res.json();
}
