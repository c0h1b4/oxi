import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe("backend URL and cookie policy", () => {
  it.each([
    ["", "", "/api", "same-origin"],
    ["/oxi", "", "/oxi/api", "same-origin"],
    ["/frontend", "http://localhost:3001/oxi/api/", "http://localhost:3001/oxi/api", "include"],
  ])("uses a common root for JSON, media, exports and sockets (%s, %s)", async (basePath, apiRoot, expected, credentials) => {
    vi.stubEnv("NEXT_PUBLIC_BASE_PATH", basePath);
    vi.stubEnv("NEXT_PUBLIC_API_URL", apiRoot);
    vi.resetModules();
    const { apiUrl, apiAccountUrl, apiCredentials } = await import("../api-url");
    expect(apiCredentials).toBe(credentials);
    for (const path of ["/messages/INBOX/1/attachments/a", "/drafts/d/attachments/a/content", "/contacts/export", "/ws"]) {
      expect(apiUrl(path)).toBe(expected + path);
      expect(apiAccountUrl(path, "account+1")).toBe(expected + path + "?account_id=account%2B1");
    }
    expect(apiAccountUrl("/contacts/export?format=vcf", "a")).toBe(expected + "/contacts/export?format=vcf&account_id=a");
  });

  it("uses the same separate-origin cookie policy for JSON API requests", async () => {
    vi.stubEnv("NEXT_PUBLIC_API_URL", "http://localhost:3001/oxi/api/");
    vi.resetModules();
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ folders: [] }) });
    vi.stubGlobal("fetch", fetchMock);
    const { apiGet } = await import("../api");
    await apiGet("/folders");
    expect(fetchMock).toHaveBeenCalledWith("http://localhost:3001/oxi/api/folders", expect.objectContaining({ credentials: "include" }));
  });

  it.each(["", "/oxi", "http://localhost:3001/oxi"])("converts inline previews back to CID without retaining origin/prefix/query (%s)", async (prefix) => {
    vi.stubEnv("NEXT_PUBLIC_BASE_PATH", prefix.startsWith("http") ? "" : prefix);
    vi.stubEnv("NEXT_PUBLIC_API_URL", prefix.startsWith("http") ? prefix + "/api" : "");
    vi.resetModules();
    const { apiAccountUrl } = await import("../api-url");
    const { inlineImagesToCid } = await import("@/components/mail/ComposeDialog/utils");
    const url = apiAccountUrl("/drafts/d/attachments/a/content", "account+1");
    expect(inlineImagesToCid(`<img src="${url}">`, "d")).toBe('<img src="cid:a">');
    expect(inlineImagesToCid('<img src="https://other.test/api/drafts/d/attachments/a/content">', "d")).toContain("https://other.test");
    expect(inlineImagesToCid(`<img src="${url}">`, null)).toContain(url);
  });
});
