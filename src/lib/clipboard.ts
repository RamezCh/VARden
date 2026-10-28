// Copying through the app: secrets are kept out of clipboard history and
// cleared after 30 seconds (see src-tauri/src/clipboard.rs).
import { invoke } from "@tauri-apps/api/core";

export async function copyText(text: string, secret: boolean): Promise<void> {
  try {
    await invoke("copy_text", { text, secret });
  } catch {
    // Outside the app (tests, a plain browser).
    await navigator.clipboard.writeText(text);
  }
}
