/**
 * Client-side search query parser for display purposes.
 * Mirrors the backend parser so we can show filter chips in the UI.
 */

export interface ParsedFilter {
  /** The operator key, e.g. "from", "to", "subject", "in", "after", "before", "date", "has" */
  operator: string;
  /** The value, e.g. "alice@example.com" */
  value: string;
  /** The raw token from the query string (used for removal) */
  raw: string;
}

export interface ParsedSearchQuery {
  /** Remaining free-text after extracting operators */
  text: string;
  /** Extracted filter tokens */
  filters: ParsedFilter[];
}

const KNOWN_OPERATORS = new Set([
  "from",
  "to",
  "subject",
  "in",
  "folder",
  "date",
  "after",
  "before",
  "has",
]);

/**
 * Tokenize a query string, respecting quoted values attached to operators.
 * E.g. `from:"John Doe" hello` => [{token: 'from:John Doe', raw: 'from:"John Doe"'}, ...]
 */
function tokenize(input: string): Array<{ token: string; raw: string }> {
  const results: Array<{ token: string; raw: string }> = [];
  let i = 0;

  while (i < input.length) {
    // Skip whitespace
    if (input[i] === " " || input[i] === "\t") {
      i++;
      continue;
    }

    let token = "";
    let raw = "";
    // Read non-whitespace characters
    while (i < input.length && input[i] !== " " && input[i] !== "\t") {
      if (input[i] === '"') {
        raw += '"';
        i++; // skip opening quote
        while (i < input.length && input[i] !== '"') {
          token += input[i];
          raw += input[i];
          i++;
        }
        if (i < input.length) {
          raw += '"';
          i++; // skip closing quote
        }
      } else {
        token += input[i];
        raw += input[i];
        i++;
      }
    }

    if (token) {
      results.push({ token, raw });
    }
  }

  return results;
}

/**
 * Normalize a search query by trimming whitespace and collapsing multiple spaces.
 */
export function normalizeSearchQuery(query: string): string {
  return query.replace(/\s+/g, " ").trim();
}

/**
 * Check if a search query is valid for commitment (length >= 2 after normalization).
 */
export function isValidCommittedSearch(query: string): boolean {
  return normalizeSearchQuery(query).length >= 2;
}

/**
 * Parse a search query string, extracting known operators into filters
 * and leaving the rest as free text.
 */
export function parseSearchQuery(input: string): ParsedSearchQuery {
  const tokens = tokenize(input);
  const filters: ParsedFilter[] = [];
  const textParts: string[] = [];

  for (const { token, raw } of tokens) {
    const colonIdx = token.indexOf(":");
    if (colonIdx > 0) {
      const op = token.slice(0, colonIdx).toLowerCase();
      const val = token.slice(colonIdx + 1);
      if (KNOWN_OPERATORS.has(op) && val.length > 0) {
        filters.push({ operator: op, value: val, raw });
        continue;
      }
    }
    textParts.push(raw);
  }

  return {
    text: textParts.join(" "),
    filters,
  };
}

/**
 * Remove a specific filter from a query string by its raw token.
 */
export function removeFilterFromQuery(
  query: string,
  filterRaw: string,
): string {
  // Replace the raw token and clean up extra whitespace
  const result = query.replace(filterRaw, "").replace(/\s+/g, " ").trim();
  return result;
}

/**
 * Get a human-readable label for a filter.
 */
export function getFilterLabel(filter: ParsedFilter): string {
  switch (filter.operator) {
    case "from":
      return `from: ${filter.value}`;
    case "to":
      return `to: ${filter.value}`;
    case "subject":
      return `subject: ${filter.value}`;
    case "in":
    case "folder":
      return `folder: ${filter.value}`;
    case "date":
      return `date: ${filter.value}`;
    case "after":
      return `after: ${filter.value}`;
    case "before":
      return `before: ${filter.value}`;
    case "has":
      return `has: ${filter.value}`;
    default:
      return `${filter.operator}: ${filter.value}`;
  }
}
