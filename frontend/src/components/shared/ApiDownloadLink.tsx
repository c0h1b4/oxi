"use client";

import type { ComponentProps } from "react";
import { toast } from "sonner";
import { apiCredentials } from "@/lib/api-url";

/** Save authenticated backend files even when the API is on another origin. */
export function ApiDownloadLink({ href, download, children, ...props }: Omit<ComponentProps<"a">, "onClick"> & { href: string; download: string }) {
  return (
    <a {...props} href={href} download={download} onClick={async (event) => {
      event.preventDefault();
      try {
        const response = await fetch(href, { credentials: apiCredentials });
        if (!response.ok) throw new Error(`Download failed (${response.status})`);
        const objectUrl = URL.createObjectURL(await response.blob());
        const link = document.createElement("a");
        link.href = objectUrl;
        link.download = download;
        document.body.append(link);
        link.click();
        link.remove();
        setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
      } catch (error) {
        toast.error(error instanceof Error ? error.message : "Download failed");
      }
    }}>
      {children}
    </a>
  );
}
