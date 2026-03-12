"use client";

import { useRef, useCallback, useEffect, useState } from "react";
import { useUiStore } from "@/stores/useUiStore";

interface EmailRendererProps {
  html: string | null;
  text: string | null;
  blockRemoteResources?: boolean;
  theme?: "light" | "dark" | "auto";
}

/**
 * Strip remote resource URLs from HTML, keeping data: and cid: URIs intact.
 * Returns the cleaned HTML and whether any remote resources were found.
 */
function stripRemoteResources(html: string): { cleaned: string; hasRemote: boolean } {
  let hasRemote = false;

  // Strip remote src attributes on img tags (keep data: and cid:)
  let cleaned = html.replace(
    /(<img\b[^>]*?\bsrc\s*=\s*)(["'])((?:https?:\/\/)[^"']*?)\2/gi,
    (_match, prefix, quote, _url) => {
      hasRemote = true;
      return `${prefix}${quote}${quote} data-blocked-src=${quote}${_url}${quote}`;
    },
  );

  // Strip remote srcset attributes
  cleaned = cleaned.replace(
    /(<img\b[^>]*?\bsrcset\s*=\s*)(["'])([^"']*?)\2/gi,
    (match, prefix, quote, value) => {
      if (/https?:\/\//i.test(value)) {
        hasRemote = true;
        return `${prefix}${quote}${quote} data-blocked-srcset=${quote}${value}${quote}`;
      }
      return match;
    },
  );

  // Strip remote background images in inline styles
  cleaned = cleaned.replace(
    /url\(\s*(["']?)(https?:\/\/[^)]*?)\1\s*\)/gi,
    (_match, quote, url) => {
      hasRemote = true;
      return `url(${quote}${quote}) /* blocked: ${url} */`;
    },
  );

  return { cleaned, hasRemote };
}

/** Check if HTML contains any remote resource URLs (http/https). */
export function hasRemoteResources(html: string | null): boolean {
  if (!html) return false;
  // Check for remote src, srcset, or background-image URLs
  return /(?:src|srcset)\s*=\s*["']https?:\/\//i.test(html) ||
    /url\(\s*["']?https?:\/\//i.test(html);
}

export function EmailRenderer({ html, text, blockRemoteResources = false, theme = "auto" }: EmailRendererProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const appTheme = useUiStore((state) => state.theme);
  const resolvedTheme = theme === "auto" ? appTheme : theme;

  const [isSystemDark, setIsSystemDark] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    setIsSystemDark(mq.matches);
    const handler = (e: MediaQueryListEvent) => setIsSystemDark(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const isDark = resolvedTheme === "dark" || (resolvedTheme === "system" && isSystemDark);

  const handleIframeLoad = useCallback(() => {
    const iframe = iframeRef.current;
    if (!iframe) return;

    try {
      const doc = iframe.contentDocument;
      const body = doc?.body;
      if (body) {
        if (isDark) {
          setTimeout(() => {
            try {
              let isEmailDark = false;
              const metaColorScheme = doc.querySelector('meta[name="color-scheme"], meta[name="supported-color-schemes"]');
              if (metaColorScheme && metaColorScheme.getAttribute('content')?.toLowerCase().includes('dark')) {
                isEmailDark = true;
              } else {
                const win = iframe.contentWindow;
                if (!win) return;

                let dominantBg = win.getComputedStyle(body).backgroundColor;
                const docW = doc.documentElement.clientWidth || body.clientWidth || win.innerWidth;
                const docH = Math.max(
                  body.scrollHeight,
                  body.offsetHeight,
                  doc.documentElement.clientHeight,
                  doc.documentElement.scrollHeight
                );
                const bodyArea = docW * docH;
                let maxArea = bodyArea * 0.5;

                const elems = body.querySelectorAll('*');
                for (let i = 0; i < elems.length; i++) {
                  const el = elems[i];
                  if (el.tagName === 'SCRIPT' || el.tagName === 'STYLE' || (el as HTMLElement).style.display === 'none') continue;
                  const style = win.getComputedStyle(el);
                  const bg = style.backgroundColor;
                  if (bg && bg !== 'rgba(0, 0, 0, 0)' && bg !== 'transparent') {
                    const rect = el.getBoundingClientRect();
                    const area = rect.width * rect.height;
                    if (area >= maxArea) {
                      maxArea = area;
                      dominantBg = bg;
                    }
                  }
                }

                const match = dominantBg.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
                if (match) {
                  const r = parseInt(match[1], 10);
                  const g = parseInt(match[2], 10);
                  const b = parseInt(match[3], 10);
                  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;

                  if (luminance < 0.5 && maxArea > bodyArea * 0.3) {
                    isEmailDark = true;
                  }
                }
              }

              if (isEmailDark && doc.documentElement) {
                doc.documentElement.classList.remove('invert-enabled');
              }
            } catch (e) {
              console.error("Error detecting email theme:", e);
            }
          }, 50);
        }

        let lastHeight = 0;
        const updateHeight = () => {
          const docEl = doc?.documentElement;
          // Calculate height with a small buffer for safety
          const height = Math.max(
            body.scrollHeight,
            body.offsetHeight,
            docEl?.clientHeight ?? 0,
            docEl?.scrollHeight ?? 0,
            docEl?.offsetHeight ?? 0
          );

          // Only update if height changed significantly to prevent resize loops
          if (Math.abs(height - lastHeight) > 2) {
            iframe.style.height = `${Math.max(height, 100)}px`;
            lastHeight = height;
          }
        };

        updateHeight();

        const images = body.querySelectorAll("img");
        images.forEach((img) => {
          if (!img.complete) {
            img.addEventListener("load", updateHeight);
            img.addEventListener("error", updateHeight);
          }
        });

        // Add ResizeObserver to watch for dynamic content or window resize changes
        if (typeof ResizeObserver !== "undefined") {
          const resizeObserver = new ResizeObserver(() => {
            updateHeight();
          });
          resizeObserver.observe(body);
        }
      }
    } catch {
      iframe.style.height = "600px";
    }
  }, [isDark]);

  if (html) {
    const processedHtml = blockRemoteResources ? stripRemoteResources(html) : { cleaned: html, hasRemote: false };
    const displayHtml = processedHtml.cleaned;

    const wrappedHtml = `<!DOCTYPE html>
<html${isDark ? ' class="invert-enabled"' : ''}>
<head>
  <meta charset="utf-8" />
  <style>
    html, body {
      background-color: white;
      color: black;
      color-scheme: light;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
        Helvetica, Arial, sans-serif;
      margin: 0;
      padding: 0px;
      box-sizing: border-box;
      overflow-y: hidden !important;
      overflow-x: auto;
    }
    img { max-width: 100%; height: auto; }
    pre { white-space: pre-wrap; word-break: break-word; }
    ${isDark ? `
    html.invert-enabled {
      filter: invert(1) hue-rotate(180deg);
    }
    html.invert-enabled img,
    html.invert-enabled picture,
    html.invert-enabled video {
      filter: invert(1) hue-rotate(180deg);
    }
    html.invert-enabled * {
      box-shadow: none !important;
      text-shadow: none !important;
    }
    ` : ""}
  </style>
</head>
<body>
  ${displayHtml}

</body>
</html>`;

    return (
      <div className={"h-full w-full overflow-auto " + (isDark ? "bg-black" : "bg-white")}>
        <iframe
          key={`${isDark ? "dark" : "light"}-${blockRemoteResources}`}
          ref={iframeRef}
          scrolling="no"
          sandbox="allow-popups allow-popups-to-escape-sandbox allow-same-origin"
          srcDoc={wrappedHtml}
          className="w-full border-none"
          style={{ minHeight: "100px" }}
          title="Email content"
          onLoad={handleIframeLoad}
        />
      </div>
    );
  }

  if (text) {
    return (
      <pre className="whitespace-pre-wrap break-words p-4 text-sm leading-relaxed text-foreground">
        {text}
      </pre>
    );
  }

  return (
    <p className="p-4 text-sm text-muted-foreground">
      No content available for this message.
    </p>
  );
}
