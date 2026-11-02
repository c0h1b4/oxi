import { describe, expect, it } from "vitest";

import { createTranslator, normalizeLocale, SUPPORTED_LOCALES } from "@/lib/i18n";
import en from "@/locales/en.json";
import es from "@/locales/es.json";

function flatKeys(value: unknown, prefix = ""): string[] {
  if (typeof value !== "object" || value === null) return [prefix];
  return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) =>
    flatKeys(child, prefix ? `${prefix}.${key}` : key),
  );
}

describe("normalizeLocale", () => {
  it("accepts a supported locale", () => {
    expect(normalizeLocale("es")).toBe("es");
  });

  it("matches a regional tag against its base language", () => {
    expect(normalizeLocale("es-419")).toBe("es");
    expect(normalizeLocale("EN-GB")).toBe("en");
  });

  it("falls back to English for anything unsupported or absent", () => {
    expect(normalizeLocale("de")).toBe("en");
    expect(normalizeLocale("")).toBe("en");
    expect(normalizeLocale(null)).toBe("en");
    expect(normalizeLocale(undefined)).toBe("en");
  });
});

describe("createTranslator", () => {
  it("returns the string for the active locale", () => {
    expect(createTranslator("es")("login.title")).toBe("Hola de nuevo");
    expect(createTranslator("en")("login.title")).toBe("Welcome back");
  });

  it("interpolates named placeholders", () => {
    const t = createTranslator("en");
    expect(t("settings.display.updateFailed", { message: "timeout" })).toBe(
      "Failed to update: timeout",
    );
  });

  it("leaves a placeholder visible when no value is supplied", () => {
    const t = createTranslator("en");
    expect(t("settings.display.updateFailed")).toBe("Failed to update: {message}");
  });
});

describe("createTranslator fallback", () => {
  const partial = { greeting: "Hola" };
  const fallback = { greeting: "Hello", farewell: "Goodbye" };

  it("falls back to the English string when the locale is missing the key", () => {
    const t = createTranslator("es", partial, fallback);
    expect(t("farewell")).toBe("Goodbye");
  });

  it("prefers the active locale when it does have the key", () => {
    const t = createTranslator("es", partial, fallback);
    expect(t("greeting")).toBe("Hola");
  });

  it("returns the key itself when neither catalog has it", () => {
    const t = createTranslator("es", partial, fallback);
    expect(t("nothing.here")).toBe("nothing.here");
  });

  it("returns the key when the path runs through a string", () => {
    const t = createTranslator("es", partial, fallback);
    expect(t("greeting.deeper")).toBe("greeting.deeper");
  });
});

describe("plural selection", () => {
  const catalog = {
    unread: { one: "{count} unread message", other: "{count} unread messages" },
  };

  it("picks the plural category for the count", () => {
    const t = createTranslator("en", catalog, catalog);
    expect(t("unread", { count: 1 })).toBe("1 unread message");
    expect(t("unread", { count: 5 })).toBe("5 unread messages");
    expect(t("unread", { count: 0 })).toBe("0 unread messages");
  });

  it("uses the locale's own plural rules", () => {
    const spanish = {
      unread: { one: "{count} mensaje sin leer", other: "{count} mensajes sin leer" },
    };
    const t = createTranslator("es", spanish, spanish);
    expect(t("unread", { count: 1 })).toBe("1 mensaje sin leer");
    expect(t("unread", { count: 2 })).toBe("2 mensajes sin leer");
  });

  it("falls back to the other form when no count is given", () => {
    const t = createTranslator("en", catalog, catalog);
    expect(t("unread")).toBe("{count} unread messages");
  });
});

describe("catalogs", () => {
  it("ships the same keys in every locale", () => {
    const expected = flatKeys(en).sort();
    expect(flatKeys(es).sort()).toEqual(expected);
  });

  it("names every supported locale", () => {
    for (const locale of SUPPORTED_LOCALES) {
      expect(en.locales[locale]).toBeTruthy();
    }
  });
});
