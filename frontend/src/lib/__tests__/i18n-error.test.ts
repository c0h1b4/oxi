import { describe, expect, it, vi } from "vitest";
import { ApiRequestError, apiPost } from "@/lib/api";
import { createTranslator } from "@/lib/i18n";
import { translateError } from "@/lib/i18n/error";

describe("localized API errors", () => {
  it("preserves machine-readable codes and existing Error messages", async () => {
    const error = { code: "UNAUTHORIZED", message: "Invalid credentials", status: 401 };
    const fetch = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(new Response(JSON.stringify({ error }), { status: 401 }));
    try {
      await expect(apiPost("/auth/login", {})).rejects.toMatchObject({ name: "ApiRequestError", ...error });
    } finally {
      fetch.mockRestore();
    }
    expect(translateError(createTranslator("es"), new ApiRequestError(error))).toBe("Correo electrónico o contraseña incorrectos");
  });

  it("keeps unknown server messages and non-API failures as fallback", () => {
    const t = createTranslator("es");
    expect(translateError(t, new ApiRequestError({ code: "NEW_CODE", message: "Specific server detail" }))).toBe("Specific server detail");
    expect(translateError(t, new Error("Network down"))).toBe("Network down");
    expect(translateError(t, null)).toBe(t("errors.unexpected"));
  });
});
