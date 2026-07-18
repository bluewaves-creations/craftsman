import path from "node:path";
import { pathToFileURL } from "node:url";

/** file:// URL of a page under site/ — no server needed (decided: the
 * playwright gates run against file://; only Lighthouse serves the dir,
 * via its own staticDistDir). */
export function siteUrl(file: string): string {
  return pathToFileURL(path.join(__dirname, "..", "site", file)).href;
}
