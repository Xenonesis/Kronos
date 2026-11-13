import { memo, useState, useCallback, useEffect, useRef } from "react";
import { View, Text, Pressable, TextInput } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Sparkles, Terminal, CornerDownLeft, Play, X, ShieldAlert } from "lucide-react-native";
import { isWeb } from "@/constants/platform";
import { invokeTauri } from "@/utils/tauri-invoke";

export interface TerminalAiCommandBarProps {
  visible: boolean;
  cwd: string;
  onClose: () => void;
  onInsertCommand: (command: string) => void;
  onExecuteCommand: (command: string) => void;
}

interface SynthesizedCommandResult {
  command: string;
  explanation: string;
  risk: "safe" | "caution" | "destructive";
  sourceLabel?: string;
}

// Offline heuristic fallback used ONLY when Tauri/LLM backend is unreachable
function synthesizeTerminalCommandFallback(prompt: string, cwd: string): SynthesizedCommandResult {
  const p = prompt.trim().toLowerCase();

  // Port queries
  const portMatch =
    p.match(/(?:port|kill port|free port|listen on|process on port)\s*[:=]?\s*(\d+)/i) ||
    p.match(/(\d{4,5})/);
  if (p.includes("port") && portMatch) {
    const port = portMatch[1];
    return {
      command: `npx kill-port ${port}`,
      explanation: `Finds and terminates any running process listening on port ${port}`,
      risk: "caution",
    };
  }

  // Docker
  if (p.includes("docker") && (p.includes("clean") || p.includes("prune"))) {
    return {
      command: "docker system prune -a --volumes",
      explanation: "Remove all unused containers, networks, images, and volumes",
      risk: "destructive",
    };
  }
  if (p.includes("docker") && p.includes("ps")) {
    return {
      command: "docker ps --format 'table {{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Ports}}'",
      explanation: "List active docker containers with clean formatting",
      risk: "safe",
    };
  }

  // Git
  if (p.includes("branch") && (p.includes("recent") || p.includes("date") || p.includes("sort"))) {
    return {
      command:
        "git branch --sort=-committerdate --format='%(committerdate:relative)%09%(refname:short)'",
      explanation: "List git branches sorted by most recent commit",
      risk: "safe",
    };
  }
  if (p.includes("undo") && (p.includes("commit") || p.includes("last commit"))) {
    return {
      command: "git reset --soft HEAD~1",
      explanation: "Undo the last commit while keeping all changes staged in working directory",
      risk: "caution",
    };
  }
  if (p.includes("stash") && (p.includes("pop") || p.includes("apply"))) {
    return {
      command: "git stash pop",
      explanation: "Applies the most recently stashed changes and drops them from stash list",
      risk: "safe",
    };
  }
  if (p.includes("clean") && p.includes("git")) {
    return {
      command: "git clean -fdn",
      explanation: "Dry-run preview of untracked files that would be removed (-n for dry-run)",
      risk: "safe",
    };
  }

  // File search
  if (p.includes("large") || p.includes("size") || p.includes("biggest")) {
    return {
      command: "git ls-files | xargs ls -lh | sort -k5 -hr | head -n 10",
      explanation: "Find the 10 largest tracked files in repository",
      risk: "safe",
    };
  }
  if (p.includes("find") || p.includes("search file")) {
    const term = p.replace(/^(find|search file|look for)\s*/i, "").trim() || "*";
    return {
      command: `find . -maxdepth 3 -iname "*${term}*" -not -path "*/node_modules/*"`,
      explanation: `Search for files matching "${term}" up to depth 3`,
      risk: "safe",
    };
  }

  // Default fallback
  return {
    command: prompt.trim(),
    explanation: `Execute command in ${cwd.split(/[/\\]/).pop() ?? cwd}`,
    risk:
      prompt.includes("rm ") || prompt.includes("del ") || prompt.includes("format")
        ? "destructive"
        : "safe",
  };
}

export const TerminalAiCommandBar = memo(function TerminalAiCommandBar({
  visible,
  cwd,
  onClose,
  onInsertCommand,
  onExecuteCommand,
}: TerminalAiCommandBarProps) {
  const { theme } = useUnistyles();
  const [prompt, setPrompt] = useState("");
  const [synthesized, setSynthesized] = useState<SynthesizedCommandResult | null>(null);
  const [isSynthesizing, setIsSynthesizing] = useState(false);
  const inputRef = useRef<TextInput>(null);

  useEffect(() => {
    if (visible) {
      setTimeout(() => {
        inputRef.current?.focus();
      }, 50);
    } else {
      setPrompt("");
      setSynthesized(null);
      setIsSynthesizing(false);
    }
  }, [visible]);

  useEffect(() => {
    const trimmed = prompt.trim();
    if (!visible || !trimmed) {
      setSynthesized(null);
      setIsSynthesizing(false);
      return;
    }

    let cancelled = false;
    setIsSynthesizing(true);
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const live = await invokeTauri<{
            command: string;
            explanation: string;
            risk: "safe" | "caution" | "destructive";
            provider: string;
            model: string;
          }>("synthesize_terminal_command", { prompt: trimmed, cwd });
          if (cancelled) return;
          if (live?.command) {
            setSynthesized({
              command: live.command,
              explanation: live.explanation,
              risk: live.risk,
              sourceLabel: `${live.provider}:${live.model}`,
            });
            setIsSynthesizing(false);
            return;
          }
        } catch {
          // Fall back to explicit offline heuristic when no active LLM provider responds
        }
        if (!cancelled) {
          const fallback = synthesizeTerminalCommandFallback(trimmed, cwd);
          setSynthesized({ ...fallback, sourceLabel: "Offline heuristic" });
          setIsSynthesizing(false);
        }
      })();
    }, 300);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [visible, prompt, cwd]);

  const handleInsert = useCallback(() => {
    if (!synthesized) return;
    onInsertCommand(synthesized.command);
    onClose();
  }, [synthesized, onInsertCommand, onClose]);

  const handleExecute = useCallback(() => {
    if (!synthesized) return;
    onExecuteCommand(synthesized.command);
    onClose();
  }, [synthesized, onExecuteCommand, onClose]);

  useEffect(() => {
    if (!visible || !isWeb) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        handleExecute();
      } else if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleInsert();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [visible, handleExecute, handleInsert, onClose]);

  if (!visible) return null;

  return (
    <View style={styles.container}>
      <View style={styles.card}>
        <View style={styles.inputRow}>
          <Sparkles size={16} color={theme.colors.accent} />
          <TextInput
            ref={inputRef}
            value={prompt}
            onChangeText={setPrompt}
            placeholder="Ask AI for terminal command (e.g. 'kill port 3000', 'undo last commit')..."
            placeholderTextColor={theme.colors.foregroundMuted}
            style={styles.textInput}
            autoCapitalize="none"
            autoCorrect={false}
          />
          <Pressable onPress={onClose} style={styles.closeBtn}>
            <X size={16} color={theme.colors.foregroundMuted} />
          </Pressable>
        </View>

        {synthesized ? (
          <View style={styles.resultBox}>
            <View style={styles.commandRow}>
              <Terminal size={14} color={theme.colors.foregroundMuted} />
              <Text style={styles.commandCode} selectable>
                {synthesized.command}
              </Text>
              {synthesized.risk === "destructive" && (
                <View style={styles.riskBadgeDestructive}>
                  <ShieldAlert size={12} color="#dc2626" />
                  <Text style={styles.riskTextDestructive}>Destructive</Text>
                </View>
              )}
              {synthesized.risk === "caution" && (
                <View style={styles.riskBadgeCaution}>
                  <Text style={styles.riskTextCaution}>Caution</Text>
                </View>
              )}
              {synthesized.sourceLabel ? (
                <Text style={styles.explanationText}>{synthesized.sourceLabel}</Text>
              ) : null}
            </View>

            <Text style={styles.explanationText}>
              {isSynthesizing ? "Synthesizing with AI provider..." : synthesized.explanation}
            </Text>

            <View style={styles.actionRow}>
              <Pressable onPress={handleInsert} style={styles.actionBtn}>
                <CornerDownLeft size={13} color={theme.colors.foreground} />
                <Text style={styles.actionBtnText}>Insert (Enter)</Text>
              </Pressable>

              <Pressable onPress={handleExecute} style={styles.executeBtn}>
                <Play size={13} color="#ffffff" />
                <Text style={styles.executeBtnText}>Run Now (Ctrl+Enter)</Text>
              </Pressable>
            </View>
          </View>
        ) : null}
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    position: "absolute",
    top: theme.spacing[2],
    left: theme.spacing[4],
    right: theme.spacing[4],
    zIndex: 9999,
    alignItems: "center",
  },
  card: {
    width: "100%",
    maxWidth: 720,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.md,
    borderWidth: 1,
    borderColor: theme.colors.borderAccent ?? theme.colors.border,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 6 },
    shadowOpacity: 0.3,
    shadowRadius: 12,
    overflow: "hidden",
  },
  inputRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    gap: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
  },
  textInput: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    padding: 0,
  },
  closeBtn: {
    padding: theme.spacing[1],
  },
  resultBox: {
    padding: theme.spacing[3],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
    backgroundColor: theme.colors.surface1,
    gap: theme.spacing[2],
  },
  commandRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: theme.spacing[1],
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  commandCode: {
    flex: 1,
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.code,
    color: theme.colors.accentBright ?? theme.colors.foreground,
  },
  riskBadgeDestructive: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: "rgba(220, 38, 38, 0.15)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  riskTextDestructive: {
    fontSize: theme.fontSize.code,
    color: "#dc2626",
    fontWeight: "600",
  },
  riskBadgeCaution: {
    backgroundColor: "rgba(245, 158, 11, 0.15)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  riskTextCaution: {
    fontSize: theme.fontSize.code,
    color: "#b45309",
    fontWeight: "600",
  },
  explanationText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
    lineHeight: 18,
  },
  actionRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "flex-end",
    gap: theme.spacing[2],
    paddingTop: theme.spacing[1],
  },
  actionBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 6,
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
  },
  actionBtnText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foreground,
    fontWeight: "500",
  },
  executeBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 6,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: theme.colors.accent,
  },
  executeBtnText: {
    fontSize: theme.fontSize.code,
    color: "#ffffff",
    fontWeight: "600",
  },
}));
