export type PluginRowAction = "reload" | "enable" | "disable" | "remove";

export function pluginRowAction(action: string | undefined): PluginRowAction | undefined {
  if (action === "reload" || action === "enable" || action === "disable" || action === "remove") {
    return action;
  }
  return undefined;
}
