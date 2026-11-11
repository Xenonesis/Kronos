/**
 * Kronos Auto-DevServer Launcher & Port Sync
 *
 * Scans workspace root for web project configurations (package.json, Cargo.toml, Vite, Next.js)
 * and detects dev server commands to launch them with 1 click from Live Preview.
 */

export interface DevServerConfig {
  name: string;
  command: string;
  defaultPort: number;
}

export function detectDevServerCommand(projectFiles: string[]): DevServerConfig | null {
  const fileSet = new Set(projectFiles.map((f) => f.toLowerCase().replace(/\\/g, "/")));

  if (fileSet.has("package.json") || Array.from(fileSet).some((f) => f.endsWith("package.json"))) {
    if (fileSet.has("vite.config.ts") || fileSet.has("vite.config.js")) {
      return { name: "Vite Dev Server", command: "npm run dev", defaultPort: 5173 };
    }
    if (
      fileSet.has("next.config.js") ||
      fileSet.has("next.config.mjs") ||
      fileSet.has("next.config.ts")
    ) {
      return { name: "Next.js Dev Server", command: "npm run dev", defaultPort: 3000 };
    }
    return { name: "Node Dev Server", command: "npm start", defaultPort: 3000 };
  }

  if (fileSet.has("cargo.toml") || Array.from(fileSet).some((f) => f.endsWith("cargo.toml"))) {
    return { name: "Rust Cargo Server", command: "cargo run", defaultPort: 8080 };
  }

  return null;
}

const LOCAL_PORT_REGEX = /https?:\/\/(?:localhost|127\.0\.0\.1):(\d{3,5})/i;

export function extractPortFromTerminalOutput(output: string): string | null {
  const match = LOCAL_PORT_REGEX.exec(output);
  return match ? match[0] : null;
}
