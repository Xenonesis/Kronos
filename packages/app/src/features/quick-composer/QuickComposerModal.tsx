import React, { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  View,
  Text,
  Modal,
  Pressable,
  type NativeSyntheticEvent,
  type TextInputKeyPressEventData,
} from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { useRouter } from "expo-router";
import {
  Sparkles,
  Zap,
  FolderGit2,
  MessageSquare,
  X,
  CornerDownLeft,
  StickyNote,
  Bot,
} from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { PressHighlight } from "@/components/ui/press-highlight";
import { Button } from "@/components/ui/button";
import { useToast } from "@/contexts/toast-context";
import { useActiveWorkspaceSelection, navigateToWorkspace } from "@/stores/navigation-active-workspace-store";
import { useSessionStore } from "@/stores/session-store";
import { useNotesStore } from "@/features/notes/use-notes";
import { NotePickerModal } from "@/features/notes/NotePickerModal";
import type { NoteRecord } from "@/features/notes/types";
import {
  AGENT_HARNESS_OPTIONS,
  type QuickComposerAgent,
  type QuickComposerTarget,
} from "./types";
import { useQuickComposerStore } from "./use-quick-composer";

const ThemedSparkles = withUnistyles(Sparkles, (theme) => ({
  color: theme.colors.accent,
}));
const ThemedClose = withUnistyles(X, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

export const QuickComposerModal = memo(function QuickComposerModal() {
  const router = useRouter();
  const toast = useToast();
  const {
    isOpen,
    target,
    selectedAgent,
    prompt,
    isDispatching,
    open,
    close,
    toggle,
    setTarget,
    setSelectedAgent,
    setPrompt,
    setIsDispatching,
    reset,
  } = useQuickComposerStore();

  const [notePickerOpen, setNotePickerOpen] = useState(false);
  const { formatNoteAsContext } = useNotesStore();

  // Active workspace info
  const activeWorkspace = useActiveWorkspaceSelection();
  const workspaceSession = useSessionStore((state) =>
    activeWorkspace
      ? state.sessions[activeWorkspace.serverId]?.workspaces.get(activeWorkspace.workspaceId)
      : null,
  );

  const workspaceName = useMemo(() => {
    if (!workspaceSession) return null;
    return (
      workspaceSession.projectCustomName ||
      workspaceSession.name ||
      workspaceSession.projectDisplayName ||
      "Active Workspace"
    );
  }, [workspaceSession]);

  // Global event & keyboard shortcut listeners
  useEffect(() => {
    if (typeof window === "undefined") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+Shift+Space or Cmd+Shift+Space
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.code === "Space" || e.key === " ")) {
        e.preventDefault();
        toggle();
        return;
      }
      // Ctrl+Shift+K
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        toggle();
        return;
      }
      // Esc when modal is open
      if (e.key === "Escape" && isOpen) {
        e.preventDefault();
        close();
        return;
      }
    };

    const handleKronosEvent = (e: Event) => {
      const customEvent = e as CustomEvent<{
        prompt?: string;
        target?: QuickComposerTarget;
        agent?: QuickComposerAgent;
      }>;
      open(customEvent.detail);
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("kronos://open-quick-composer", handleKronosEvent);
    window.addEventListener("kronos:open-quick-composer", handleKronosEvent);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("kronos://open-quick-composer", handleKronosEvent);
      window.removeEventListener("kronos:open-quick-composer", handleKronosEvent);
    };
  }, [close, isOpen, open, toggle]);

  const handlePromptChange = useCallback(
    (text: string) => {
      setPrompt(text);
    },
    [setPrompt],
  );

  const handleAttachNote = useCallback(
    (note: NoteRecord) => {
      const formatted = formatNoteAsContext(note);
      setPrompt(prompt ? `${prompt}\n\n${formatted}` : formatted);
      toast.show(`Attached note "${note.title}"`);
    },
    [formatNoteAsContext, prompt, setPrompt, toast],
  );

  const handleDispatch = useCallback(async () => {
    const trimmed = prompt.trim();
    if (!trimmed || isDispatching) return;

    setIsDispatching(true);

    try {
      if (target === "workspace" && activeWorkspace) {
        // Submit into workspace
        navigateToWorkspace({
          serverId: activeWorkspace.serverId,
          workspaceId: activeWorkspace.workspaceId,
        });

        // Dispatch composer event
        if (typeof window !== "undefined") {
          window.dispatchEvent(
            new CustomEvent("kronos:dispatch-composer", {
              detail: {
                text: trimmed,
                agent: selectedAgent,
              },
            }),
          );
        }

        toast.show(
          `Dispatched to ${selectedAgent.toUpperCase()} in ${workspaceName || "Workspace"}`,
        );
      } else {
        // General Chat / Scratchpad
        router.push({
          pathname: "/new",
          params: {
            provider: selectedAgent,
          },
        });

        if (typeof window !== "undefined") {
          window.dispatchEvent(
            new CustomEvent("kronos:insert-composer", {
              detail: {
                text: trimmed,
              },
            }),
          );
        }

        toast.show(`Dispatched to General Chat (${selectedAgent.toUpperCase()})`);
      }

      reset();
      close();
    } catch {
      setIsDispatching(false);
    }
  }, [
    activeWorkspace,
    close,
    isDispatching,
    prompt,
    reset,
    router,
    selectedAgent,
    setIsDispatching,
    target,
    toast,
    workspaceName,
  ]);

  const handleInsert = useCallback(async () => {
    const trimmed = prompt.trim();
    if (!trimmed || isDispatching) return;

    try {
      if (target === "workspace" && activeWorkspace) {
        navigateToWorkspace({
          serverId: activeWorkspace.serverId,
          workspaceId: activeWorkspace.workspaceId,
        });

        if (typeof window !== "undefined") {
          window.dispatchEvent(
            new CustomEvent("kronos:insert-composer", {
              detail: {
                text: trimmed,
              },
            }),
          );
        }

        toast.show(`Inserted into ${workspaceName || "Workspace"}`);
      } else {
        router.push({
          pathname: "/new",
          params: {
            provider: selectedAgent,
          },
        });

        if (typeof window !== "undefined") {
          window.dispatchEvent(
            new CustomEvent("kronos:insert-composer", {
              detail: {
                text: trimmed,
              },
            }),
          );
        }

        toast.show(`Inserted into General Chat (${selectedAgent.toUpperCase()})`);
      }

      reset();
      close();
    } catch {
      // ignore
    }
  }, [
    activeWorkspace,
    close,
    isDispatching,
    prompt,
    reset,
    router,
    selectedAgent,
    target,
    toast,
    workspaceName,
  ]);

  const handleKeyPress = useCallback(
    (e: NativeSyntheticEvent<TextInputKeyPressEventData>) => {
      if (e.nativeEvent.key === "Enter") {
        // If Shift is pressed, multiline happens natively.
        // If Enter alone, trigger dispatch!
        const shiftPressed = (e as unknown as { shiftKey?: boolean }).shiftKey;
        if (!shiftPressed) {
          void handleDispatch();
        }
      }
    },
    [handleDispatch],
  );

  if (!isOpen) return null;

  return (
    <Modal visible={isOpen} transparent animationType="fade" onRequestClose={close}>
      <View style={styles.backdrop}>
        <Pressable style={styles.dismissOverlay} onPress={close} />
        <View style={styles.card} testID="quick-composer-modal">
          {/* Spotlight Header */}
          <View style={styles.header}>
            <View style={styles.spotlightBadge}>
              <ThemedSparkles size={15} />
              <Text style={styles.spotlightBadgeText}>Spotlight Composer</Text>
            </View>

            {/* Target Selector Switch */}
            <View style={styles.targetSwitch}>
              <PressHighlight
                style={[
                  styles.targetSwitchBtn,
                  target === "workspace" && styles.targetSwitchBtnActive,
                ]}
                onPress={() => setTarget("workspace")}
              >
                <FolderGit2
                  size={13}
                  color={target === "workspace" ? "#38bdf8" : "#888888"}
                />
                <Text
                  style={[
                    styles.targetSwitchText,
                    target === "workspace" && styles.targetSwitchTextActive,
                  ]}
                  numberOfLines={1}
                >
                  {workspaceName ? workspaceName : "Workspace"}
                </Text>
              </PressHighlight>

              <PressHighlight
                style={[
                  styles.targetSwitchBtn,
                  target === "general" && styles.targetSwitchBtnActive,
                ]}
                onPress={() => setTarget("general")}
              >
                <MessageSquare
                  size={13}
                  color={target === "general" ? "#a855f7" : "#888888"}
                />
                <Text
                  style={[
                    styles.targetSwitchText,
                    target === "general" && styles.targetSwitchTextActive,
                  ]}
                >
                  Scratchpad
                </Text>
              </PressHighlight>
            </View>

            {/* Esc hint + close */}
            <View style={styles.headerRight}>
              <View style={styles.escBadge}>
                <Text style={styles.escBadgeText}>ESC</Text>
              </View>
              <PressHighlight style={styles.closeBtn} onPress={close}>
                <ThemedClose size={16} />
              </PressHighlight>
            </View>
          </View>

          {/* Agent / Harness Selector Bar */}
          <View style={styles.agentBar}>
            <Text style={styles.agentBarLabel}>Agent:</Text>
            <View style={styles.agentOptions}>
              {AGENT_HARNESS_OPTIONS.map((opt) => {
                const isSelected = selectedAgent === opt.id;
                return (
                  <PressHighlight
                    key={opt.id}
                    style={[
                      styles.agentChip,
                      isSelected && {
                        borderColor: opt.color,
                        backgroundColor: `${opt.color}1a`,
                      },
                    ]}
                    onPress={() => setSelectedAgent(opt.id)}
                  >
                    <View
                      style={[
                        styles.agentDot,
                        { backgroundColor: opt.color },
                        isSelected && styles.agentDotActive,
                      ]}
                    />
                    <Text
                      style={[
                        styles.agentChipText,
                        isSelected && { color: "#ffffff", fontWeight: "600" },
                      ]}
                    >
                      {opt.label}
                    </Text>
                    <Text style={styles.agentBadge}>{opt.badge}</Text>
                  </PressHighlight>
                );
              })}
            </View>
          </View>

          {/* Prompt Input Box */}
          <View style={styles.inputArea}>
            <AdaptiveTextInput
              initialValue={prompt}
              placeholder={`Ask ${selectedAgent.toUpperCase()}, draft architecture, or instruct an edit...`}
              onChangeText={handlePromptChange}
              onKeyPress={handleKeyPress}
              multiline
              autoFocus
              style={styles.textInput}
            />
          </View>

          {/* Spotlight Actions Footer */}
          <View style={styles.footer}>
            <View style={styles.footerLeft}>
              <PressHighlight
                style={styles.attachNoteBtn}
                onPress={() => setNotePickerOpen(true)}
              >
                <StickyNote size={13} color="#888888" />
                <Text style={styles.attachNoteText}>Attach Note (@note)</Text>
              </PressHighlight>
              <Text style={styles.shortcutHint}>
                <Text style={styles.shortcutKey}>Shift+Enter</Text> newline
              </Text>
            </View>

            <View style={styles.footerRight}>
              <Button
                size="sm"
                variant="outline"
                onPress={handleInsert}
                disabled={isDispatching || prompt.trim().length === 0}
                testID="quick-composer-insert-button"
              >
                <CornerDownLeft size={13} color="#888888" />
                <Text style={styles.insertBtnText}>Insert</Text>
              </Button>
              <Button
                size="sm"
                onPress={handleDispatch}
                disabled={isDispatching || prompt.trim().length === 0}
                testID="quick-composer-dispatch-button"
              >
                <Zap size={14} color="#ffffff" />
                <Text style={styles.dispatchBtnText}>
                  {isDispatching ? "Dispatching..." : "Dispatch (Enter)"}
                </Text>
              </Button>
            </View>
          </View>
        </View>
      </View>

      <NotePickerModal
        visible={notePickerOpen}
        onClose={() => setNotePickerOpen(false)}
        onSelectNote={handleAttachNote}
      />
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.72)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  dismissOverlay: {
    ...StyleSheet.absoluteFillObject,
  },
  card: {
    width: 680,
    maxWidth: "94%",
    backgroundColor: "rgba(18, 20, 24, 0.96)",
    borderRadius: 16,
    borderWidth: theme.borderWidth[1],
    borderColor: "rgba(255, 255, 255, 0.12)",
    shadowColor: "#000000",
    shadowOffset: { width: 0, height: 12 },
    shadowOpacity: 0.55,
    shadowRadius: 24,
    elevation: 24,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: "rgba(255, 255, 255, 0.08)",
  },
  spotlightBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  spotlightBadgeText: {
    fontSize: 13,
    fontWeight: "700",
    color: theme.colors.foreground,
    letterSpacing: 0.3,
  },
  targetSwitch: {
    flexDirection: "row",
    backgroundColor: "rgba(255, 255, 255, 0.06)",
    borderRadius: theme.borderRadius.md,
    padding: 2,
    gap: 2,
  },
  targetSwitchBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: theme.spacing[3],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
  },
  targetSwitchBtnActive: {
    backgroundColor: theme.colors.surface0,
    shadowColor: "#000000",
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.2,
    shadowRadius: 4,
  },
  targetSwitchText: {
    fontSize: 12,
    color: theme.colors.foregroundMuted,
    maxWidth: 160,
  },
  targetSwitchTextActive: {
    color: theme.colors.foreground,
    fontWeight: "600",
  },
  headerRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  escBadge: {
    backgroundColor: "rgba(255, 255, 255, 0.08)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  escBadgeText: {
    fontSize: 10,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
  },
  closeBtn: {
    padding: 4,
  },
  agentBar: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[3],
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[2],
    backgroundColor: "rgba(0, 0, 0, 0.2)",
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: "rgba(255, 255, 255, 0.05)",
  },
  agentBarLabel: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  agentOptions: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 6,
  },
  agentChip: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: theme.borderRadius.md,
    borderWidth: theme.borderWidth[1],
    borderColor: "rgba(255, 255, 255, 0.08)",
    backgroundColor: "rgba(255, 255, 255, 0.03)",
  },
  agentDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
  },
  agentDotActive: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  agentChipText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  agentBadge: {
    fontSize: 9,
    color: "rgba(255, 255, 255, 0.4)",
    backgroundColor: "rgba(255, 255, 255, 0.05)",
    paddingHorizontal: 4,
    paddingVertical: 1,
    borderRadius: 3,
  },
  inputArea: {
    minHeight: 130,
    maxHeight: 260,
    padding: theme.spacing[4],
  },
  textInput: {
    flex: 1,
    fontSize: 14,
    lineHeight: 22,
    color: theme.colors.foreground,
    textAlignVertical: "top",
    padding: 0,
  },
  footer: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderTopWidth: theme.borderWidth[1],
    borderTopColor: "rgba(255, 255, 255, 0.08)",
    backgroundColor: "rgba(0, 0, 0, 0.15)",
  },
  footerLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[3],
  },
  attachNoteBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: "rgba(255, 255, 255, 0.05)",
  },
  attachNoteText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  shortcutHint: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  shortcutKey: {
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  footerRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  insertBtnText: {
    fontSize: 12,
    fontWeight: "500",
    color: theme.colors.foregroundMuted,
  },
  dispatchBtnText: {
    fontSize: 12,
    fontWeight: "600",
    color: "#ffffff",
  },
}));
