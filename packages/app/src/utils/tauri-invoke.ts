/**
 * Shared typed bridge for invoking native Rust Tauri commands when running
 * inside the Kronos desktop runtime, returning null in pure web/test sandboxes.
 */

export interface TauriInternals {
  invoke: <T = unknown>(command: string, args?: Record<string, unknown>) => Promise<T>;
}

export function getTauriInternals(): TauriInternals | null {
  if (typeof window === "undefined") {
    return null;
  }
  const internals = (
    window as unknown as {
      __TAURI_INTERNALS__?: TauriInternals;
    }
  ).__TAURI_INTERNALS__;
  return internals && typeof internals.invoke === "function" ? internals : null;
}

export async function invokeTauri<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T | null> {
  const tauri = getTauriInternals();
  if (!tauri) {
    return null;
  }
  return tauri.invoke<T>(command, args);
}
