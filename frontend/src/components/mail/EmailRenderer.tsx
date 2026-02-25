"use client";

import { useRef, useCallback } from "react";

interface EmailRendererProps {
  html: string | null;
  text: string | null;
  blockRemoteResources?: boolean;
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

export function EmailRenderer({ html, text, blockRemoteResources = false }: EmailRendererProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const handleIframeLoad = useCallback(() => {
    const iframe = iframeRef.current;
    if (!iframe) return;

    try {
      const body = iframe.contentDocument?.body;
      if (body) {
        // Set initial height, then observe for changes (e.g. images loading)
        const updateHeight = () => {
          const height = body.scrollHeight;
          iframe.style.height = `${height}px`;
        };

        updateHeight();

        // Re-measure after images and other resources finish loading
        const images = body.querySelectorAll("img");
        images.forEach((img) => {
          if (!img.complete) {
            img.addEventListener("load", updateHeight);
            img.addEventListener("error", updateHeight);
          }
        });
      }
    } catch {
      // If we can't access contentDocument (shouldn't happen with srcDoc),
      // fall back to a reasonable minimum height
      iframe.style.height = "600px";
    }
  }, []);

  if (html) {
    const processedHtml = blockRemoteResources ? stripRemoteResources(html) : { cleaned: html, hasRemote: false };
    const displayHtml = processedHtml.cleaned;

    const wrappedHtml = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
        Helvetica, Arial, sans-serif;
      font-size: 14px;
      line-height: 1.5;
      color: #1a1a1a;
      word-wrap: break-word;
      overflow-wrap: break-word;
      margin: 0;
      padding: 16px;
    }
    img { max-width: 100%; height: auto; }
    a { color: #2563eb; }
    pre { white-space: pre-wrap; word-break: break-word; }
    table { max-width: 100%; }
  </style>
</head>
<body>${displayHtml}</body>
</html>`;

    return (
      <iframe
        ref={iframeRef}
        sandbox="allow-popups allow-popups-to-escape-sandbox"
        srcDoc={wrappedHtml}
        className="h-full w-full border-none"
        title="Email content"
        onLoad={handleIframeLoad}
      />
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
