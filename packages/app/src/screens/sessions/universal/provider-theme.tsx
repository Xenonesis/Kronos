import { Bot, Code, Cpu, Layers, Sparkles, Terminal } from "lucide-react-native";

export function getProviderTheme(provider: string): {
  name: string;
  color: string;
  bg: string;
} {
  switch (provider.toLowerCase()) {
    case "antigravity":
      return { name: "Antigravity", color: "#38bdf8", bg: "rgba(56, 189, 248, 0.12)" };
    case "omp":
      return { name: "OMP", color: "#a855f7", bg: "rgba(168, 85, 247, 0.12)" };
    case "claude":
    case "claudecode":
      return { name: "Claude Code", color: "#f59e0b", bg: "rgba(245, 158, 11, 0.12)" };
    case "codex":
      return { name: "Codex", color: "#10b981", bg: "rgba(16, 185, 129, 0.12)" };
    case "opencode":
      return { name: "OpenCode", color: "#06b6d4", bg: "rgba(6, 182, 212, 0.12)" };
    case "pi":
      return { name: "Pi", color: "#ec4899", bg: "rgba(236, 72, 153, 0.12)" };
    default:
      return { name: "Kronos", color: "#6366f1", bg: "rgba(99, 102, 241, 0.12)" };
  }
}

export function ProviderGlyph({ provider, color }: { provider: string; color: string }) {
  switch (provider.toLowerCase()) {
    case "antigravity":
      return <Sparkles size={11} color={color} />;
    case "omp":
      return <Bot size={11} color={color} />;
    case "claude":
    case "claudecode":
      return <Sparkles size={11} color={color} />;
    case "codex":
      return <Terminal size={11} color={color} />;
    case "opencode":
      return <Code size={11} color={color} />;
    case "pi":
      return <Cpu size={11} color={color} />;
    default:
      return <Layers size={11} color={color} />;
  }
}
