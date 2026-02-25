"use client";

import { useRef, useCallback } from "react";

interface EmailRendererProps {
  html: string | null;
  text: string | null;
}

export function EmailRenderer({ html, text }: EmailRendererProps) {
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
      padding: 0;
    }
    img { max-width: 100%; height: auto; }
    a { color: #2563eb; }
    pre { white-space: pre-wrap; word-break: break-word; }
    table { max-width: 100%; }
  </style>
</head>
<body>${html}</body>
</html>`;

    return (
      <iframe
        ref={iframeRef}
        sandbox="allow-popups allow-popups-to-escape-sandbox"
        srcDoc={wrappedHtml}
        style={{ width: "100%", border: "none", minHeight: 200 }}
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
