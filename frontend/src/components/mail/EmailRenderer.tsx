"use client";

import { useRef, useCallback, useEffect, useState, useReducer, useMemo } from "react";
import { useUiStore } from "@/stores/useUiStore";

interface EmailRendererProps {
  html: string | null;
  text: string | null;
  blockRemoteResources?: boolean;
  theme?: "light" | "dark" | "auto";
  emailTheme?: 'light' | 'dark' | 'transparent' | 'adaptive';
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

export function EmailRenderer({
  html,
  text,
  blockRemoteResources = false,
  theme = "auto",
  emailTheme
}: EmailRendererProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [isReady, setIsReady] = useReducer((_: boolean, ready: boolean) => ready, false);

  const appTheme = useUiStore((state) => state.theme);
  const effectiveAnimationMode = useUiStore((state) => state.effectiveAnimationMode);
  const resolvedTheme = theme === "auto" ? appTheme : theme;
  const [prevTrack, setPrevTrack] = useState<{
    surface: "html" | "text" | "empty" | null;
    textSignature: string | null;
    richStreamSession: number;
    shouldAnimate: boolean;
  }>({ surface: null, textSignature: null, richStreamSession: 0, shouldAnimate: false });

  const [isSystemDark, setIsSystemDark] = useState(() => {
    if (typeof window === "undefined") return false;
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  });

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => setIsSystemDark(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const isDark = resolvedTheme === "dark" || (resolvedTheme === "system" && isSystemDark);

   const shouldInvert = useMemo(() => {
    // Adaptive emails handle their own dark mode via CSS media queries
    if (emailTheme === 'adaptive') return false;
    // Transparent emails use direct theme application, not inversion
    if (!emailTheme || emailTheme === 'transparent') return false;

    const isAppDark = resolvedTheme === "dark" || (resolvedTheme === "system" && isSystemDark);

    return (emailTheme === 'dark' && !isAppDark) || (emailTheme === 'light' && isAppDark);
  }, [emailTheme, resolvedTheme, isSystemDark]);

  // Compute a stable key for the iframe — when this changes, React remounts the iframe
  const iframeKey = `${shouldInvert ? "inverted" : "normal"}-${isDark ? "dark" : "light"}-${blockRemoteResources}`;

  // Track iframe key changes to reset ready state
  const iframeKeyRef = useRef(iframeKey);
  useEffect(() => {
    if (iframeKeyRef.current !== iframeKey) {
      iframeKeyRef.current = iframeKey;
      setIsReady(false);
    }
  }, [iframeKey]);

  const handleIframeLoad = useCallback(() => {
    const iframe = iframeRef.current;
    if (!iframe) return;

    try {
      const doc = iframe.contentDocument;
      const body = doc?.body;
      if (body) {
        // Use the container height as a minimum so email bg fills the pane
        const containerHeight = iframe.parentElement?.clientHeight ?? 0;
        let lastHeight = 0;
        const updateHeight = () => {
          const docEl = doc?.documentElement;
          const contentHeight = Math.max(
            body.scrollHeight,
            body.offsetHeight,
            docEl?.clientHeight ?? 0,
            docEl?.scrollHeight ?? 0,
            docEl?.offsetHeight ?? 0
          );
          const height = Math.max(contentHeight, containerHeight);

          if (Math.abs(height - lastHeight) > 2) {
            iframe.style.height = `${height}px`;
            lastHeight = height;
          }
        };

        updateHeight();
        setIsReady(true);

        const images = body.querySelectorAll("img");
        images.forEach((img) => {
          if (!img.complete) {
            img.addEventListener("load", updateHeight);
            img.addEventListener("error", updateHeight);
          }
        });

        if (typeof ResizeObserver !== "undefined") {
          const resizeObserver = new ResizeObserver(() => {
            updateHeight();
          });
          resizeObserver.observe(body);
        }
      }
    } catch {
      iframe.style.height = "600px";
      setIsReady(true);
    }
  }, []);

  // Memoize the heavy HTML processing — regex replacements, CSS generation, etc.
  // This only re-runs when the actual inputs change, not on every render.
  const wrappedHtml = useMemo(() => {
    if (!html) return null;

    const processedHtml = blockRemoteResources ? stripRemoteResources(html) : { cleaned: html, hasRemote: false };
    const displayHtml = processedHtml.cleaned;

    // Extract the email's own background color from body tag (for padding bg matching)
    let emailBgColor: string | null = null;
    
    // Strip nested <html>, <head>, <body> tags from email but keep their content.
    // This prevents the browser from creating a second document root that could
    // bypass our inversion filter. We keep <style> tags from the email intact.
    let cleanedHtml = displayHtml
      .replace(/<html[^>]*>/gi, '')
      .replace(/<\/html>/gi, '')
      .replace(/<head[^>]*>/gi, '')
      .replace(/<\/head>/gi, '')
      // Strip email's own color-scheme meta tags — we set our own in the wrapper
      .replace(/<meta[^>]*(?:color-scheme|supported-color-schemes)[^>]*>/gi, '')
      // Strip email's own CSS color-scheme and supported-color-schemes declarations
      // Use negative lookbehind to avoid matching 'prefers-color-scheme' inside @media queries
      .replace(/(?<![-])(?:supported-)?color-schemes?\s*:[^;}"']+[;]?/gi, '')
      .replace(/<body[^>]*>/gi, (match: string) => {
        // Extract inline style from body tag and apply it to a wrapper div
        const styleMatch = match.match(/style\s*=\s*"([^"]+)"/i) || match.match(/style\s*=\s*'([^']+)'/i);
        const bgMatch = match.match(/bgcolor\s*=\s*["']([^"']+)["']/i);
        let wrapperStyle = styleMatch ? styleMatch[1] : '';
        if (bgMatch && !wrapperStyle.includes('background')) {
          wrapperStyle += (wrapperStyle ? '; ' : '') + 'background-color: ' + bgMatch[1];
        }
        // Extract the background color for iframe padding matching.
        // Prefer background-color over background shorthand (which may have gradients).
        const bgcMatch = wrapperStyle.match(/background-color\s*:\s*([^;!]+)/i);
        const bgShorthandMatch = wrapperStyle.match(/background\s*:\s*([^;!]+)/i);
        if (bgcMatch) {
          emailBgColor = bgcMatch[1].trim();
        } else if (bgShorthandMatch) {
          // Extract a hex or rgb color from background shorthand (skip gradients)
          const colorInShorthand = bgShorthandMatch[1].match(/(#[0-9a-fA-F]{3,8}|rgb\([^)]+\)|rgba\([^)]+\))/);
          if (colorInShorthand) {
            emailBgColor = colorInShorthand[1];
          }
        } else if (bgMatch) {
          emailBgColor = bgMatch[1].trim();
        }
        return wrapperStyle ? `<div style="${wrapperStyle}">` : '<div>';
      })
      .replace(/<\/body>/gi, '</div>');

    // For adaptive emails, we can't rely on @media(prefers-color-scheme:dark)
    // because browsers evaluate it against OS preference, not the iframe's
    // color-scheme meta tag. Instead:
    // - In dark mode: extract rules from @media dark blocks and inject as unconditional CSS
    // - In light mode: strip the blocks entirely so they don't fire from OS dark mode
    let adaptiveDarkCSS = '';
    if (emailTheme === 'adaptive') {
      const darkMediaRe = /@media\s*\([^)]*prefers-color-scheme\s*:\s*dark[^)]*\)\s*\{([^{}]*(?:\{[^{}]*\}[^{}]*)*)\}/gi;
      
      if (isDark) {
        // Extract inner CSS rules and collect them for unconditional injection
        const extractedRules: string[] = [];
        cleanedHtml = cleanedHtml.replace(darkMediaRe, (_match, innerCSS: string) => {
          extractedRules.push(innerCSS.trim());
          return '/* dark mode CSS extracted and applied unconditionally */';
        });
        if (extractedRules.length > 0) {
          adaptiveDarkCSS = `\n/* Adaptive dark mode CSS applied unconditionally */\n${extractedRules.join('\n')}`;
        }
      } else {
        cleanedHtml = cleanedHtml.replace(darkMediaRe,
          '/* dark mode CSS stripped for light mode */'
        );
      }
    }

    // If body tag had a plain white background (#ffffff/white), the actual visible
    // background is likely on the first wrapper table/td with a non-white bg.
    // Prefer that for padding matching since white on body is often just a boilerplate reset.
    const bodyBgIsWhite = emailBgColor && /^(#fff(fff)?|white)$/i.test(emailBgColor);

    // Find the first non-white background from wrapper tables/tds
    const wrapperBgRe = /<(?:table|td)\b[^>]*(?:style\s*=\s*"[^"]*background(?:-color)?\s*:\s*([^;!"]+)[^"]*"|bgcolor\s*=\s*["']([^"']+)["'])[^>]*>/gi;
    let wrapperBgColor: string | null = null;
    for (const wrapperMatch of cleanedHtml.matchAll(wrapperBgRe)) {
      const color = (wrapperMatch[1] || wrapperMatch[2] || '').trim();
      if (color && !/^(#fff(fff)?|white)$/i.test(color)) {
        wrapperBgColor = color;
        break;
      }
    }

    if ((bodyBgIsWhite || !emailBgColor) && wrapperBgColor) {
      emailBgColor = wrapperBgColor;
    }

    // App theme colors (must match globals.css --background / --foreground)
    const appDarkBg = '#141110';   // hsl(20 10% 7%)
    const appDarkFg = '#f5f3f1';   // hsl(20 10% 95%) — approximate foreground
    const appLightBg = '#ffffff';  // hsl(0 0% 100%)
    const appLightFg = '#0a0a0a';  // hsl(0 0% 4%) — approximate foreground

    // Set color-scheme to match the visual appearance of the iframe content.
    // This affects browser chrome like scrollbars and form controls.
    let iframeColorScheme: string;
    if (emailTheme === 'adaptive') {
      // Adaptive emails respond to the app's theme via their own CSS
      iframeColorScheme = isDark ? 'dark' : 'light';
    } else if (shouldInvert) {
      // Inverted content: visual result is opposite of email's original theme
      iframeColorScheme = emailTheme === 'dark' ? 'light' : 'dark';
    } else {
      // Not inverting: transparent uses app theme, others match email theme
      if (!emailTheme || emailTheme === 'transparent') {
        iframeColorScheme = isDark ? 'dark' : 'light';
      } else {
        iframeColorScheme = emailTheme === 'dark' ? 'dark' : 'light';
      }
    }

    // Determine iframe background-color and text color based on scenario:
    // - Inverted: use body's OWN bg (not wrapper override) so white→black inversion is clean
    // - Transparent/text-only: inherit app theme colors directly (no inversion)
    // - Adaptive: let email handle it, but set fallback to app colors
    // - Normal (no invert, not transparent): use emailBgColor (may be wrapper override) for padding match
    let iframeBg: string;
    let iframeFg: string;

    if (shouldInvert) {
      // When inverting, use emailBgColor (which includes the wrapper non-white override)
      // so the 16px padding bg matches the email's visible content bg through the filter.
      // e.g. light-10 wrapper bg #edf2f7 inverts to #090e13 — padding must match.
      if (emailTheme === 'light') {
        iframeBg = emailBgColor || appLightBg;
        iframeFg = appLightFg;
      } else {
        iframeBg = emailBgColor || appDarkBg;
        iframeFg = appDarkFg;
      }
    } else if (!emailTheme || emailTheme === 'transparent') {
      // Transparent: no inversion, directly apply app theme
      iframeBg = isDark ? appDarkBg : appLightBg;
      iframeFg = isDark ? appDarkFg : appLightFg;
    } else if (emailTheme === 'adaptive') {
      // Adaptive: the email's own CSS sets backgrounds (we've injected dark rules unconditionally).
      // Try to extract the email's dark body background from the injected CSS for padding matching.
      // Fall back to app theme colors.
      if (isDark && adaptiveDarkCSS) {
        // Look for the first background color in the adaptive dark CSS rules
        const darkBgMatch = adaptiveDarkCSS.match(/background(?:-color)?\s*:\s*(#[0-9a-fA-F]{3,8}|rgb\([^)]+\))/i);
        iframeBg = darkBgMatch ? darkBgMatch[1].trim() : appDarkBg;
      } else {
        iframeBg = isDark ? appDarkBg : appLightBg;
      }
      iframeFg = isDark ? appDarkFg : appLightFg;
    } else {
      // Normal: email theme matches app theme — use email's own bg for seamless padding
      iframeBg = emailBgColor || (isDark ? appDarkBg : appLightBg);
      iframeFg = isDark ? appDarkFg : appLightFg;
    }

    // For transparent emails in dark mode, we need to force text color since
    // these emails have inline color:black that would be invisible on dark bg.
    // We can't use CSS inversion (that produces pure black bg, not app dark bg),
    // so we override text colors directly.
    const isTransparentDark = (!emailTheme || emailTheme === 'transparent') && isDark;
    const transparentDarkCSS = isTransparentDark ? `
    /* Force light text on dark bg for transparent emails */
    body, body * {
      color: ${iframeFg} !important;
    }
    /* Force transparent backgrounds so email wrapper divs/tables don't
       cover our dark bg (e.g. Odoo's background-color:white on wrapper table) */
    body div, body table, body td, body th, body tr, body tbody {
      background-color: transparent !important;
      background: transparent !important;
    }
    /* Preserve link colors */
    a, a * {
      color: #6ba3e8 !important;
    }
    ` : '';

    return `<!DOCTYPE html>
<html class="${shouldInvert ? 'invert' : ''}">
<head>
  <meta charset="utf-8" />
  <meta name="color-scheme" content="${iframeColorScheme}" />
  <style>
    html {
      background-color: ${iframeBg};
      color: ${iframeFg};
      color-scheme: ${iframeColorScheme} !important;
    }
    html, body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
        Helvetica, Arial, sans-serif;
      margin: 0 !important;
      padding: 0 !important;
      height: auto !important;
      width: auto !important;
      min-height: 100% !important;
      box-sizing: border-box;
      overflow-y: hidden !important;
      overflow-x: auto;
    }
    body {
      padding: 16px !important;
      background-color: ${iframeBg};
      color: ${iframeFg};
    }
    *, *::before, *::after {
      box-sizing: border-box;
    }
    img { max-width: 100%; height: auto; }
    pre { white-space: pre-wrap; word-break: break-word; }
    a { overflow-wrap: break-word; word-break: break-all; }
    ${transparentDarkCSS}

    /* Inversion filter - inverts all rendered colors including text */
    html.invert {
      filter: invert(1) hue-rotate(180deg);
    }

    /* Double-invert media to preserve original appearance */
    html.invert img,
    html.invert picture,
    html.invert video,
    html.invert canvas,
    html.invert svg,
    html.invert [style*="background-image"] {
      filter: invert(1) hue-rotate(180deg);
    }

    /* Remove shadows that look wrong when inverted */
    html.invert * {
      box-shadow: none !important;
      text-shadow: none !important;
    }

    /* Override CSS system colors (windowtext, window, buttonface, etc.)
       that don't render correctly in modern browsers and can make text
       invisible against certain backgrounds */
    [style*="windowtext" i] { color: inherit !important; }
    [style*="WindowText" i] { color: inherit !important; }
    [style*="window;" i] { background-color: inherit !important; }
    [style*="buttonface" i] { background-color: inherit !important; }
    [style*="InfoBackground" i] { background-color: inherit !important; }
    ${adaptiveDarkCSS}
  </style>
</head>
<body>
  ${cleanedHtml}

  <!-- Re-assert layout rules after email content to override email <style> blocks
       that may set html/body { height: 100%; margin: 0; width: 100%; } etc. -->
  <style>
    html {
      color-scheme: ${iframeColorScheme} !important;
    }
    html, body {
      margin: 0 !important;
      padding: 0 !important;
      height: auto !important;
      width: auto !important;
      min-height: 100% !important;
      overflow-y: hidden !important;
    }
    body {
      padding: 16px !important;
    }
    ${transparentDarkCSS}
    ${adaptiveDarkCSS}
  </style>
</body>
</html>`;
  }, [html, blockRemoteResources, emailTheme, shouldInvert, isDark]);

  const surface: "html" | "text" | "empty" = wrappedHtml ? "html" : text ? "text" : "empty";
  const textSignature = text ? `${text.length}:${text}` : null;

  // Adjust state during render (React-recommended pattern for "previous value" tracking).
  // See: https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
  const needsUpdate =
    prevTrack.surface !== surface ||
    (surface === "text" && prevTrack.textSignature !== textSignature);

  let shouldStartRichStream = prevTrack.shouldAnimate;
  let richStreamSession = prevTrack.richStreamSession;

  if (needsUpdate) {
    shouldStartRichStream =
      surface === "text" &&
      effectiveAnimationMode === "rich" &&
      (
        prevTrack.surface == null ||
        prevTrack.surface === "html" ||
        prevTrack.textSignature !== textSignature
      );

    if (shouldStartRichStream) {
      richStreamSession = prevTrack.richStreamSession + 1;
    }

    setPrevTrack({
      surface,
      textSignature: surface === "text" ? textSignature : null,
      richStreamSession,
      shouldAnimate: shouldStartRichStream,
    });
  }

  if (wrappedHtml) {
    return (
      <div className="h-full w-full overflow-auto bg-background relative">
        {!isReady && (
          <div className="space-y-2 p-4">
            <div className="h-4 w-full animate-pulse rounded bg-muted" />
            <div className="h-4 w-full animate-pulse rounded bg-muted" />
            <div className="h-4 w-5/6 animate-pulse rounded bg-muted" />
            <div className="h-4 w-full animate-pulse rounded bg-muted" />
            <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
          </div>
        )}
        {/*
         * SECURITY: iframe sandbox attribute rationale
         *
         * allow-same-origin — Required so the parent can access iframe.contentDocument
         *   to measure content height (ResizeObserver, scrollHeight), attach image
         *   load listeners, and dynamically resize the iframe. Without it, the
         *   browser blocks all cross-document DOM access and the auto-height logic
         *   in handleIframeLoad breaks entirely.
         *
         * allow-popups / allow-popups-to-escape-sandbox — Lets mailto: and http(s)
         *   links open in new tabs/windows as users expect.
         *
         * RISK: allow-same-origin combined with allow-scripts would let embedded
         *   content access the parent page's cookies, localStorage, and DOM — a
         *   full same-origin escalation. This is safe ONLY because allow-scripts
         *   is deliberately omitted: the sandbox blocks all script execution
         *   (<script> tags, inline handlers, javascript: URLs, etc.), so even if
         *   email HTML contains malicious JS it cannot run.
         *
         * If a future browser bug or spec change weakens the script-blocking
         *   guarantee, this would become exploitable. Mitigations considered:
         *
         *   - Removing allow-same-origin and using postMessage for height: would
         *     require injecting a <script> into srcdoc (reintroducing allow-scripts),
         *     which is strictly worse — it trades a theoretical risk for a concrete one.
         *
         *   - Serving email HTML from a separate origin (e.g., blob: URL on a
         *     different subdomain): adds deployment complexity and CORS overhead
         *     for marginal benefit given the current sandbox guarantees.
         *
         *   - Using shadow DOM instead of an iframe: does not provide the same
         *     style/script isolation as the sandbox attribute.
         *
         * Conclusion: the current sandbox="allow-popups allow-popups-to-escape-sandbox
         *   allow-same-origin" (without allow-scripts) is the best tradeoff. It
         *   provides full script isolation while preserving the DOM access needed
         *   for responsive iframe sizing. Re-evaluate if the HTML Sandbox spec
         *   changes or if we move to a separate-origin rendering service.
         */}
        <iframe
          key={iframeKey}
          ref={iframeRef}
          scrolling="no"
          sandbox="allow-popups allow-popups-to-escape-sandbox allow-same-origin"
          srcDoc={wrappedHtml}
          className="w-full border-none transition-opacity duration-150"
          style={{
            minHeight: "100%",
            opacity: isReady ? 1 : 0,
            position: isReady ? "static" : "absolute",
            pointerEvents: isReady ? "auto" : "none",
          }}
          title="Email content"
          onLoad={handleIframeLoad}
        />
      </div>
    );
  }

  if (text) {
    if (effectiveAnimationMode === "off") {
      return (
        <pre
          data-testid="email-renderer-plaintext-static"
          className="whitespace-pre-wrap break-words p-4 text-sm leading-relaxed text-foreground"
        >
          {text}
        </pre>
      );
    }

    if (effectiveAnimationMode === "rich") {
      const lines = text.split(/\r?\n/);
      const isLargeBody = text.length > 6000 || lines.length > 180;
      const shouldAnimateRich = shouldStartRichStream;

      if (isLargeBody) {
        return (
          <pre
            data-testid="email-renderer-plaintext-large-reveal"
            data-stream-session={String(richStreamSession)}
            className="whitespace-pre-wrap break-words p-4 text-sm leading-relaxed text-foreground transition-opacity duration-200"
            style={{
              animationName: shouldAnimateRich ? "email-plaintext-container-reveal" : "none",
              animationDuration: shouldAnimateRich ? "220ms" : "0ms",
              animationTimingFunction: "cubic-bezier(0.2, 0, 0, 1)",
              animationFillMode: "both",
            }}
          >
            {text}
          </pre>
        );
      }

      return (
        <pre
          data-testid="email-renderer-plaintext-rich-stream"
          data-stream-session={String(richStreamSession)}
          className="whitespace-pre-wrap break-words p-4 text-sm leading-relaxed text-foreground"
        >
          {lines.map((line, index) => (
            <span
              key={`${richStreamSession}-${index}`}
              data-testid="email-renderer-plaintext-line"
              style={{
                display: "inline-block",
                width: "100%",
                animationName: shouldAnimateRich ? "email-plaintext-line-reveal" : "none",
                animationDuration: shouldAnimateRich ? "260ms" : "0ms",
                animationTimingFunction: "cubic-bezier(0.2, 0, 0, 1)",
                animationFillMode: "both",
                animationDelay: shouldAnimateRich ? `${Math.min(index * 18, 320)}ms` : "0ms",
              }}
            >
              {line}
              {index < lines.length - 1 ? "\n" : ""}
            </span>
          ))}
        </pre>
      );
    }

    const simpleAnimationName =
      effectiveAnimationMode === "medium"
        ? "email-plaintext-medium-reveal"
        : "email-plaintext-subtle-reveal";
    const simpleAnimationDuration = effectiveAnimationMode === "medium" ? "170ms" : "110ms";

    return (
      <pre
        data-testid="email-renderer-plaintext-simple-transition"
        className="whitespace-pre-wrap break-words p-4 text-sm leading-relaxed text-foreground transition-opacity duration-150"
        style={{
          animationName: simpleAnimationName,
          animationDuration: simpleAnimationDuration,
          animationTimingFunction: "cubic-bezier(0.2, 0, 0, 1)",
          animationFillMode: "both",
        }}
      >
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
