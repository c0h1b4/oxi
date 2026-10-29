"use client";

import { useId, useState } from "react";
import { useSubjectSuggestions } from "@/hooks/useSubjectSuggestions";

interface SubjectInputProps {
  value: string;
  onChange: (value: string) => void;
  enabled?: boolean;
}

export function SubjectInput({ value, onChange, enabled = true }: SubjectInputProps) {
  const listId = useId();
  const [focused, setFocused] = useState(false);
  const [dismissedValue, setDismissedValue] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const suggestions = useSubjectSuggestions(value, enabled && focused);
  const open = focused && enabled && dismissedValue !== value && suggestions.length > 0;
  const activeIndex = Math.min(selectedIndex, suggestions.length - 1);

  function select(subject: string) {
    onChange(subject);
    setDismissedValue(subject);
    setSelectedIndex(-1);
  }

  return (
    <div className="relative min-w-0 flex-1">
      <input
        type="text"
        role="combobox"
        aria-label="Subject"
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={open && activeIndex >= 0 ? `${listId}-${activeIndex}` : undefined}
        autoComplete="off"
        value={value}
        placeholder="Subject"
        className="w-full bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground/50"
        onChange={(event) => {
          onChange(event.target.value);
          setSelectedIndex(-1);
          setDismissedValue(null);
        }}
        onFocus={() => { setFocused(true); setDismissedValue(null); }}
        onBlur={() => { setFocused(false); setSelectedIndex(-1); }}
        onKeyDown={(event) => {
          if (event.ctrlKey || event.metaKey || event.altKey || event.nativeEvent.isComposing) return;
          if (!open) return;
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            setSelectedIndex(event.key === "ArrowDown"
              ? (activeIndex + 1) % suggestions.length
              : (activeIndex <= 0 ? suggestions.length - 1 : activeIndex - 1));
          } else if (event.key === "Enter" && activeIndex >= 0) {
            event.preventDefault();
            select(suggestions[activeIndex]);
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            setDismissedValue(value);
          }
        }}
      />
      {open && (
        <ul id={listId} role="listbox" aria-label="Subject suggestions"
          className="absolute left-0 top-full z-50 mt-1 max-h-64 w-full overflow-y-auto rounded-lg border border-border bg-popover py-1 shadow-lg">
          {suggestions.map((subject, index) => (
            <li key={subject} id={`${listId}-${index}`} role="option" aria-selected={activeIndex === index}
              className={`cursor-pointer break-words px-3 py-2 text-sm ${activeIndex === index ? "bg-accent text-accent-foreground" : "text-popover-foreground hover:bg-accent/50"}`}
              onMouseEnter={() => setSelectedIndex(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => select(subject)}>
              {subject}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
