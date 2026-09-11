export function isPdfAttachment(contentType: string, filename?: string | null): boolean {
  return contentType.split(";")[0].trim().toLowerCase() === "application/pdf"
    || !!filename?.toLowerCase().endsWith(".pdf");
}
