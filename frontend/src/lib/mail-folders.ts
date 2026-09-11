import type { Folder } from "@/types/folder";

export function findJunkFolder(folders: Folder[]): string | undefined {
  const special = folders.find(folder => folder.attributes?.some(attr => attr.toLowerCase() === "\\junk"));
  if (special) return special.name;
  return folders.find(folder => {
    const leaf = folder.delimiter ? folder.name.split(folder.delimiter).at(-1) : folder.name;
    return ["spam", "junk", "junk email", "junk e-mail"].includes(leaf?.toLowerCase() ?? "");
  })?.name;
}
