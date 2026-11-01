import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const builtinPlugins = [
  "antigravity-provider",
  "claude-usage-source",
  "codex-usage-source",
  "copilot-usage-source",
  "cursor-usage-source",
  "grok-usage-source",
  "hoplite-provider",
  "kimi-usage-source",
  "minimax-usage-source",
  "muse-provider",
  "opencode-go-usage-source",
  "worktree-include",
  "zai-usage-source",
] as const;

export function resolveBuiltinPluginsRoot(moduleUrl: string | URL = import.meta.url): string {
  const moduleDir = path.dirname(fileURLToPath(moduleUrl));
  const archiveSegment = `${path.sep}app.asar${path.sep}`;
  const archiveIndex = moduleDir.indexOf(archiveSegment);
  if (archiveIndex !== -1) {
    // esbuild runs outside Electron's filesystem shim and cannot read archive entries.
    return path.join(moduleDir.slice(0, archiveIndex), "builtin-plugins");
  }
  const candidates = [
    path.resolve(moduleDir, "..", "..", "..", "builtin-plugins"),
    path.resolve(moduleDir, "..", "..", "..", "..", "..", "..", "plugins"),
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? candidates[0]!;
}

export interface BuiltinPlugin {
  id: string;
  directory: string;
}

export class BuiltinPluginLoader {
  readonly ids: ReadonlySet<string>;

  constructor(
    private readonly root = resolveBuiltinPluginsRoot(),
    private readonly list: readonly string[] = builtinPlugins,
  ) {
    this.ids = new Set(list);
  }

  loadPartitioned(start: (plugin: BuiltinPlugin) => Promise<void>): {
    eager: Promise<void>;
    deferred: Promise<void>;
  } {
    const eagerPlugins: BuiltinPlugin[] = [];
    const deferredPlugins: BuiltinPlugin[] = [];

    for (const id of this.list) {
      const plugin: BuiltinPlugin = { id, directory: path.join(this.root, id) };
      if (id.endsWith("-usage-source") || id === "usage-source") {
        deferredPlugins.push(plugin);
      } else {
        eagerPlugins.push(plugin);
      }
    }

    const eager = Promise.all(eagerPlugins.map(start)).then(() => undefined);
    const deferred = Promise.all(deferredPlugins.map(start)).then(() => undefined);

    return { eager, deferred };
  }

  async load(start: (plugin: BuiltinPlugin) => Promise<void>): Promise<void> {
    const { eager, deferred } = this.loadPartitioned(start);
    await Promise.all([eager, deferred]);
  }
}
