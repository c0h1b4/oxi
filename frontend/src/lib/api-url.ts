/** Backend API root, including /api (and any reverse-proxy prefix). */
const API_BASE = (
  process.env.NEXT_PUBLIC_API_URL
  || (process.env.NEXT_PUBLIC_BASE_PATH || "") + "/api"
).replace(/\/+$/, "");

export const apiCredentials: RequestCredentials = process.env.NEXT_PUBLIC_API_URL
  ? "include" : "same-origin";

export function apiUrl(path: string): string {
  return `${API_BASE}/${path.replace(/^\/+/, "")}`;
}

/** Media elements and new tabs cannot send X-Active-Account headers. */
export function apiAccountUrl(path: string, accountId: string | null): string {
  const url = apiUrl(path);
  if (!accountId) return url;
  const separator = url.includes("?") ? "&" : "?";
  return `${url}${separator}account_id=${encodeURIComponent(accountId)}`;
}
