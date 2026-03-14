"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAutocomplete } from "@/hooks/useAutocomplete";

interface RecipientInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  inputRef?: React.Ref<HTMLInputElement>;
}

function HighlightedText({ html }: { html: string }) {
  const parts = useMemo(() => {
    if (!html) return [];
    const result: Array<{ text: string; highlighted: boolean }> = [];
    const regex = /<mark>(.*?)<\/mark>/g;
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    match = regex.exec(html);
    while (match !== null) {
      if (match.index > lastIndex) {
        result.push({ text: html.slice(lastIndex, match.index), highlighted: false });
      }
      result.push({ text: match[1], highlighted: true });
      lastIndex = match.index + match[0].length;
      match = regex.exec(html);
    }

    if (lastIndex < html.length) {
      result.push({ text: html.slice(lastIndex), highlighted: false });
    }

    return result;
  }, [html]);

  if (parts.length === 0) return null;

  return (
    <>
      {parts.map((part, i) =>
        part.highlighted ? (
          <span key={`${i}-${part.text.slice(0, 10)}`} className="bg-primary/20 font-semibold">
            {part.text}
          </span>
        ) : (
          <span key={`${i}-${part.text.slice(0, 10)}`}>{part.text}</span>
        )
      )}
    </>
  );
}

export function RecipientInput({
  value,
  onChange,
  placeholder,
  inputRef,
}: RecipientInputProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  // Track the query for which the dropdown was dismissed, so typing a new
  // query naturally re-opens it without setState-in-effect.
  const [dismissedQuery, setDismissedQuery] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Extract the text after the last comma as the autocomplete query
  const query = value.split(",").pop()?.trim() ?? "";
  const { results, isLoading } = useAutocomplete(query);

  const hasResults = results.length > 0 && query.length >= 2;

  // Dropdown shows when there are results and the user hasn't dismissed this exact query
  const showDropdown = hasResults && dismissedQuery !== query;

  const selectSuggestion = useCallback(
    (suggestion: { email: string; name?: string }) => {
      const parts = value.split(",");
      parts.pop();
      const formatted = suggestion.name
        ? `${suggestion.name} <${suggestion.email}>`
        : suggestion.email;
      parts.push(formatted);
      onChange(parts.join(", ") + ", ");
      setDismissedQuery(query);
    },
    [value, onChange, query],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!showDropdown || results.length === 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter" || e.key === "Tab") {
        if (showDropdown && results.length > 0) {
          e.preventDefault();
          selectSuggestion(results[selectedIndex].item);
        }
      } else if (e.key === "Escape") {
        setDismissedQuery(query);
      }
    },
    [showDropdown, results, selectedIndex, selectSuggestion, query],
  );

  // Close dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setDismissedQuery(query);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [query]);

  return (
    <div ref={containerRef} className="relative flex-1">
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          setSelectedIndex(0);
        }}
        onKeyDown={handleKeyDown}
        onFocus={() => setDismissedQuery(null)}
        placeholder={placeholder}
        className="w-full bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground/50"
      />
      {showDropdown && results.length > 0 && (
        <div className="absolute left-0 top-full z-50 mt-1 w-72 rounded-lg border border-border bg-popover shadow-lg">
          {isLoading && (
            <div className="px-3 py-2 text-xs text-muted-foreground">
              Searching...
            </div>
          )}
          {results.map((result, i) => (
            <button
              key={`${result.item.email}-${result.item.source}`}
              type="button"
              className={`flex w-full flex-col px-3 py-2 text-left text-sm transition-colors ${
                i === selectedIndex
                  ? "bg-accent text-accent-foreground"
                  : "text-popover-foreground hover:bg-accent/50"
              }`}
              onMouseEnter={() => setSelectedIndex(i)}
              onMouseDown={(e) => {
                e.preventDefault(); // prevent input blur
                selectSuggestion(result.item);
              }}
            >
              <span className="font-medium">
                {result.item.name ? (
                  <HighlightedText html={result.highlightedName ?? result.item.name} />
                ) : (
                  <HighlightedText html={result.highlightedEmail} />
                )}
              </span>
              {result.item.name && (
                <span className="text-xs text-muted-foreground">
                  <HighlightedText html={result.highlightedEmail} />
                </span>
              )}
              {result.item.source === "known" && (
                <span className="mt-0.5 text-xs text-muted-foreground/60">
                  From email history
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
