/**
 * The save-picker shape for one action's export. The backend writes a zip
 * bundle when files are attached and plain JSON when fileless, so the
 * picker must offer the matching name and filter — a zip saved under a
 * `.json` name restores (magic-byte sniffing) but misleads about what the
 * file is. One backup document format either way: the zip holds the same
 * envelope as `action.json` plus the raw bytes under `files/`.
 */
export interface ActionExportTarget {
  defaultPath: string;
  filters: { name: string; extensions: string[] }[];
}

export function actionExportTarget(
  actionName: string,
  fileCount: number,
): ActionExportTarget {
  if (fileCount > 0) {
    return {
      defaultPath: `${actionName}.zip`,
      filters: [{ name: "Sprout action bundle", extensions: ["zip"] }],
    };
  }
  return {
    defaultPath: `${actionName}.json`,
    filters: [{ name: "Sprout backup", extensions: ["json"] }],
  };
}
