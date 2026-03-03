import { useEffect, type RefObject } from "react";

/**
 * Calls `handler` when a mousedown event occurs outside the referenced element.
 * The listener is only attached when `enabled` is true.
 */
export function useClickOutside(
  ref: RefObject<HTMLElement | null>,
  handler: () => void,
  enabled: boolean = true,
) {
  useEffect(() => {
    if (!enabled) return;
    function onMouseDown(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        handler();
      }
    }
    document.addEventListener("mousedown", onMouseDown);
    return () => document.removeEventListener("mousedown", onMouseDown);
  }, [ref, handler, enabled]);
}
