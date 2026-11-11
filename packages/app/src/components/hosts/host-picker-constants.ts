export const ADD_HOST_OPTION_ID = "__add_host__";
export const ALL_HOSTS_OPTION_ID = "__all_hosts__";
export const ENABLE_BUILT_IN_DAEMON_OPTION_ID = "__enable_built_in_daemon__";

export function getHostPickerLabel(
  hosts: Array<{ label: string; serverId: string }>,
  value: string,
  config?: { includeAllHost?: boolean; includeAddHost?: boolean },
): string {
  if (config?.includeAllHost && value === ALL_HOSTS_OPTION_ID) {
    return "All hosts";
  }
  if (config?.includeAddHost && value === ADD_HOST_OPTION_ID) {
    return "Add host";
  }
  const host = hosts.find((h) => h.serverId === value);
  if (host) {
    const lower = host.label.toLowerCase();
    if (lower === "localhost" || lower === "127.0.0.1") {
      return "Local (This Device)";
    }
    return host.label;
  }
  return config?.includeAllHost ? "All hosts" : "Host";
}
