import { getDesktopHost, type DesktopDialogBridge } from "./host";

export async function pickDirectory(
  dialog: DesktopDialogBridge | null = getDesktopHost()?.dialog ?? null,
): Promise<string | null> {
  const open = dialog?.open;
  if (typeof open === "function") {
    const selection = await open({
      directory: true,
      multiple: false,
      createDirectory: true,
    });
    if (selection === null) {
      return null;
    }
    if (typeof selection === "string") {
      return selection;
    }
    if (Array.isArray(selection) && typeof selection[0] === "string") {
      return selection[0];
    }
    throw new Error("Unexpected directory picker response.");
  }

  const host = getDesktopHost();
  if (typeof host?.invoke === "function") {
    const selection = await host.invoke("pick_folder");
    if (selection === null || selection === undefined) {
      return null;
    }
    if (typeof selection === "string") {
      return selection;
    }
    throw new Error("Unexpected directory picker response.");
  }

  throw new Error("Desktop dialog open() is unavailable in this environment.");
}
