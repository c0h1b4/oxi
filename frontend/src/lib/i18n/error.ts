import { ApiRequestError } from "@/lib/api";
import type { Translator } from "@/lib/i18n";

/**
 * Turn an API failure into a translated message.
 *
 * The backend returns English messages with a stable machine-readable `code`,
 * so the code is what gets translated. An unrecognised code keeps the server's
 * own message, which is more useful than a generic string.
 */
export function translateError(t: Translator, error: unknown): string {
  if (error instanceof ApiRequestError && error.code) {
    const key = `errors.${error.code}`;
    const translated = t(key, { message: error.message });
    if (translated !== key) return translated;
  }
  if (error instanceof Error && error.message) return error.message;
  return t("errors.unexpected");
}
