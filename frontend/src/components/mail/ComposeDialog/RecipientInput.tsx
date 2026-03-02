"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useAutocomplete } from "@/hooks/useContacts";

interface RecipientInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  inputRef?: React.Ref<HTMLInputElement>;
}

export function RecipientInput({
  value,
  onChange,
  placeholder,
  inputRef,
}: RecipientInputProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [showDropdown, setShowDropdown] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Extract the text after the last comma as the autocomplete query
  const query = value.split(",").pop()?.trim() ?? "";
  const { data: suggestions } = useAutocomplete(query);

  const hasSuggestions = !!suggestions && suggestions.length > 0 && query.length >= 2;

  useEffect(() => {
    setShowDropdown(hasSuggestions);
    setSelectedIndex(0);
  }, [hasSuggestions]);

  const selectSuggestion = useCallback(
    (suggestion: { email: string; name: string }) => {
      const parts = value.split(",");
      parts.pop(); // remove the partial text
      const formatted = suggestion.name
        ? `${suggestion.name} <${suggestion.email}>`
        : suggestion.email;
      parts.push(formatted);
      onChange(parts.join(", ") + ", ");
      setShowDropdown(false);
    },
    [value, onChange],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!showDropdown || !suggestions || suggestions.length === 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, suggestions.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter" || e.key === "Tab") {
        if (showDropdown && suggestions.length > 0) {
          e.preventDefault();
          selectSuggestion(suggestions[selectedIndex]);
        }
      } else if (e.key === "Escape") {
        setShowDropdown(false);
      }
    },
    [showDropdown, suggestions, selectedIndex, selectSuggestion],
  );

  // Close dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  return (
    <div ref={containerRef} className="relative flex-1">
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        onFocus={() => hasSuggestions && setShowDropdown(true)}
        placeholder={placeholder}
        className="w-full bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground/50"
      />
      {showDropdown && suggestions && suggestions.length > 0 && (
        <div className="absolute left-0 top-full z-50 mt-1 w-72 rounded-lg border border-border bg-popover shadow-lg">
          {suggestions.map((s, i) => (
            <button
              key={s.email}
              type="button"
              className={`flex w-full flex-col px-3 py-2 text-left text-sm transition-colors ${
                i === selectedIndex
                  ? "bg-accent text-accent-foreground"
                  : "text-popover-foreground hover:bg-accent/50"
              }`}
              onMouseEnter={() => setSelectedIndex(i)}
              onMouseDown={(e) => {
                e.preventDefault(); // prevent input blur
                selectSuggestion(s);
              }}
            >
              {s.name && (
                <span className="font-medium">{s.name}</span>
              )}
              <span className="text-xs text-muted-foreground">{s.email}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
