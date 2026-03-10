import { invoke } from "@tauri-apps/api/core";

export function installErrorCapture() {
  window.addEventListener("error", (event) => {
    const message = event instanceof ErrorEvent
      ? event.message
      : (event as Event & { message?: string }).message || "unknown";
    invoke("write_debug_log", { message: `window error: ${message}` }).catch(() => {});
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason = (event as PromiseRejectionEvent).reason;
    const text = reason instanceof Error ? String(reason) : String(reason ?? "unknown");
    invoke("write_debug_log", { message: `unhandledrejection: ${text}` }).catch(() => {});
  });
}
