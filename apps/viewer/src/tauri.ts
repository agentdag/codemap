// Tauri v2 integration, isolated so web mode never depends on a Tauri runtime.
// All calls are guarded by `isTauri()`; in the browser the "Open folder" action
// is hidden and these functions are never invoked.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { IrDocument } from "./ir";

/** True only inside the Tauri native webview. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Show a folder picker, run the engine (`extract_repo`) on the chosen path via
 * the `analyze_repo` command, and return the parsed IR. Returns `null` when the
 * user cancels; throws the backend error string on failure.
 */
export async function analyzeFolder(): Promise<IrDocument | null> {
  const picked = await open({
    directory: true,
    multiple: false,
    title: "Select a repository folder",
  });
  if (typeof picked !== "string") return null; // cancelled
  const json = await invoke<string>("analyze_repo", { path: picked });
  return JSON.parse(json) as IrDocument;
}
