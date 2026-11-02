import en from "@/locales/en.json";
import es from "@/locales/es.json";

export const SUPPORTED_LOCALES = ["en", "es"] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

/** The English catalog is the source of truth: every other locale must match its shape. */
export type Catalog = typeof en;

const catalogs: Record<Locale, Catalog> = { en, es };

export type TranslationVars = Record<string, string | number>;

type PluralForms = Partial<Record<Intl.LDMLPluralRule, string>>;
type TranslationNode = string | PluralForms | { [key: string]: TranslationNode };

export function isLocale(value: string): value is Locale {
  return (SUPPORTED_LOCALES as readonly string[]).includes(value);
}

/**
 * Resolve an arbitrary language tag to a supported locale.
 * Accepts "es", "es-ES" and "ES"; anything unknown falls back to English.
 */
export function normalizeLocale(value: string | null | undefined): Locale {
  const base = value?.toLowerCase().split("-")[0] ?? "";
  return isLocale(base) ? base : DEFAULT_LOCALE;
}

export function localeName(locale: Locale): string {
  return catalogs[locale].locales[locale];
}

function lookup(catalog: TranslationNode, key: string): TranslationNode | undefined {
  let node: TranslationNode | undefined = catalog;
  for (const part of key.split(".")) {
    if (typeof node !== "object" || node === null) return undefined;
    node = (node as { [key: string]: TranslationNode })[part];
  }
  return node;
}

function selectForm(
  entry: TranslationNode,
  vars: TranslationVars,
  plural: Intl.PluralRules,
): string | undefined {
  if (typeof entry === "string") return entry;
  if (typeof entry !== "object" || entry === null) return undefined;
  const forms = entry as PluralForms;
  const count = vars.count;
  if (typeof count !== "number") return forms.other;
  return forms[plural.select(count)] ?? forms.other;
}

/** Replace {placeholders}; an unknown placeholder is left visible rather than printed as "undefined". */
function interpolate(template: string, vars: TranslationVars): string {
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in vars ? String(vars[name]) : match,
  );
}

export type Translator = (key: string, vars?: TranslationVars) => string;

/**
 * Build a translation function for a locale.
 *
 * A key missing from the active catalog falls back to the English string, and a
 * key missing from both is returned verbatim so the gap is visible in the UI
 * instead of rendering as an empty element.
 */
export function createTranslator(
  locale: Locale,
  catalog: TranslationNode = catalogs[locale],
  fallbackCatalog: TranslationNode = catalogs[DEFAULT_LOCALE],
): Translator {
  const plural = new Intl.PluralRules(locale);
  const fallbackPlural = new Intl.PluralRules(DEFAULT_LOCALE);

  return function t(key, vars = {}) {
    const entry = lookup(catalog, key);
    const template = entry === undefined ? undefined : selectForm(entry, vars, plural);
    if (template !== undefined) return interpolate(template, vars);

    const fallback = lookup(fallbackCatalog, key);
    const fallbackTemplate =
      fallback === undefined ? undefined : selectForm(fallback, vars, fallbackPlural);
    if (fallbackTemplate !== undefined) return interpolate(fallbackTemplate, vars);

    return key;
  };
}
