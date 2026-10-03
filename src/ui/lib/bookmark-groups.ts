export interface Foldered {
  folder: string | null;
}

export interface FolderGroup<T extends Foldered> {
  name: string;
  label: string;
  items: T[];
}

export const NO_FOLDER_LABEL = "Not in a folder";

function folderOf(item: Foldered): string {
  return item.folder?.trim() ?? "";
}

export function folderNames<T extends Foldered>(items: T[]): string[] {
  const names = new Set(items.map(folderOf).filter((name) => name !== ""));
  return [...names].sort((a, b) => a.localeCompare(b));
}

export function folderGroups<T extends Foldered>(items: T[]): FolderGroup<T>[] {
  const groups = folderNames(items).map((name) => ({
    name,
    label: name,
    items: items.filter((item) => folderOf(item) === name),
  }));
  const loose = items.filter((item) => folderOf(item) === "");
  if (loose.length > 0) groups.push({ name: "", label: NO_FOLDER_LABEL, items: loose });
  return groups;
}
