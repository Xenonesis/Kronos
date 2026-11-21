import type { HostConnection } from "@/types/host-connection";
import { getDesktopHost } from "@/desktop/host";
import { getManagedDesktopStatus, getManagedDesktopTransport } from "./desktop-daemon";

function connectionListen(connection: HostConnection): string | null {
  switch (connection.type) {
    case "directTcp":
      return connection.endpoint;
    case "directSocket":
      return connection.path === "ipc://embedded" ? connection.path : `unix://${connection.path}`;
    case "directPipe":
      return `pipe://${connection.path}`;
    case "remoteSsh":
    case "routed":
    case "relay":
      return null;
  }
}

// Native rechecks actual caller, daemon identity, ownership and current target on every hello.
export async function readDesktopManagedLocalCredential(
  connection: HostConnection,
  serverId: string,
  invoke: (input: { listen: string; serverId: string }) => Promise<unknown> = async (input) =>
    getDesktopHost()?.invoke?.("desktop_local_credential", input),
): Promise<string | undefined> {
  if (!serverId || getManagedDesktopStatus()?.serverId !== serverId) return undefined;
  getManagedDesktopTransport(connection, serverId);
  const listen = connectionListen(connection);
  if (!listen) return undefined;
  const credential = await invoke({ listen, serverId });
  return typeof credential === "string" ? credential : undefined;
}
