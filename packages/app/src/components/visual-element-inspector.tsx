import React, { memo, useState, useCallback } from "react";
import { View, Text, Pressable, TextInput } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { MousePointerClick, Sparkles, X, Layers } from "lucide-react-native";
import { useSessionStore } from "@/stores/session-store";
import { useDraftStore } from "@/stores/draft-store";
import { useToast } from "@/contexts/toast-context";

export interface InspectedElement {
  tagName: string;
  id?: string;
  className?: string;
  textSnippet?: string;
  rect?: { top: number; left: number; width: number; height: number };
}

export interface VisualElementInspectorProps {
  active: boolean;
  previewUrl: string;
  onToggleActive: () => void;
  onElementSelected?: (element: InspectedElement) => void;
}

export const VisualElementInspector = memo(function VisualElementInspector({
  active,
  previewUrl,
  onToggleActive,
  onElementSelected,
}: VisualElementInspectorProps) {
  const { theme } = useUnistyles();
  const toast = useToast();
  const [selectedElement, setSelectedElement] = useState<InspectedElement | null>(null);
  const [promptText, setPromptText] = useState("");

  const handleApplyEdit = useCallback(() => {
    if (!promptText.trim()) return;

    const sessions = useSessionStore.getState().sessions;
    const serverIds = Object.keys(sessions);
    const serverId = serverIds[0];
    const session = serverId ? sessions[serverId] : undefined;
    const activeAgent = session?.agents ? Array.from(session.agents.values())[0] : undefined;

    if (!serverId || !activeAgent) {
      toast.show("No active agent available to apply visual edit", { variant: "error" });
      return;
    }

    const draftKey = `agent:${serverId}:${activeAgent.id}`;
    const elementDescription = selectedElement
      ? `Tag: <${selectedElement.tagName.toLowerCase()}${
          selectedElement.id ? ` id="${selectedElement.id}"` : ""
        }${selectedElement.className ? ` class="${selectedElement.className}"` : ""}>\n` +
        (selectedElement.textSnippet ? `Text Content: "${selectedElement.textSnippet}"\n` : "")
      : "Visual element from live preview\n";

    const fullPrompt =
      `### 🎨 Visual UI Edit Request (Live Preview: ${previewUrl})\n` +
      `Target Element:\n\`\`\`html\n${elementDescription}\`\`\`\n` +
      `User Instruction: ${promptText}\n\n` +
      `Please locate the matching component source file, apply the requested styling/layout edits, and keep tests passing.`;

    useDraftStore.getState().replaceDraftText({ draftKey, text: fullPrompt });
    toast.show("Visual edit prompt sent to composer", { variant: "success" });
    setPromptText("");
    setSelectedElement(null);
    onToggleActive();
  }, [promptText, selectedElement, previewUrl, toast, onToggleActive]);

  return (
    <View style={styles.container} pointerEvents="box-none">
      {/* Inspector Toggle Button */}
      <Pressable
        onPress={onToggleActive}
        style={[styles.toggleButton, active && styles.toggleButtonActive]}
        testID="visual-inspector-toggle"
      >
        <MousePointerClick size={14} color={active ? "#ffffff" : theme.colors.foregroundMuted} />
        <Text style={[styles.toggleText, active && styles.toggleTextActive]}>
          {active ? "Pick Element Active" : "Inspect Element"}
        </Text>
      </Pressable>

      {/* Popover when an element is inspected */}
      {active && selectedElement ? (
        <View style={styles.promptPopover} pointerEvents="auto">
          <View style={styles.popoverHeader}>
            <View style={styles.elementTagRow}>
              <Layers size={14} color={theme.colors.accent} />
              <Text style={styles.elementTag}>&lt;{selectedElement.tagName.toLowerCase()}&gt;</Text>
            </View>
            <Pressable onPress={() => setSelectedElement(null)} style={styles.closePopoverButton}>
              <X size={14} color={theme.colors.foregroundMuted} />
            </Pressable>
          </View>

          <TextInput
            style={styles.promptInput}
            value={promptText}
            onChangeText={setPromptText}
            placeholder="Describe the UI change (e.g. make button purple with rounded corners)..."
            placeholderTextColor={theme.colors.foregroundMuted}
            multiline
          />

          <View style={styles.popoverFooter}>
            <Pressable onPress={handleApplyEdit} style={styles.applyButton}>
              <Sparkles size={12} color="#ffffff" />
              <Text style={styles.applyButtonText}>Edit with Kronos</Text>
            </Pressable>
          </View>
        </View>
      ) : null}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    position: "relative",
    flexDirection: "row",
    alignItems: "center",
  },
  toggleButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: 6,
    backgroundColor: theme.colors.surface1,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  toggleButtonActive: {
    backgroundColor: theme.colors.accent,
    borderColor: theme.colors.accent,
  },
  toggleText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    fontWeight: "500",
  },
  toggleTextActive: {
    color: "#ffffff",
    fontWeight: "600",
  },
  promptPopover: {
    position: "absolute",
    bottom: 36,
    right: 0,
    width: 320,
    backgroundColor: theme.colors.surface0,
    borderRadius: 10,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 12,
    boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
    zIndex: 1000,
  },
  popoverHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 8,
  },
  elementTagRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  elementTag: {
    fontFamily: "monospace",
    fontSize: 12,
    fontWeight: "700",
    color: theme.colors.foreground,
  },
  closePopoverButton: {
    padding: 2,
  },
  promptInput: {
    backgroundColor: theme.colors.surface1,
    borderRadius: 6,
    padding: 8,
    fontSize: 12,
    color: theme.colors.foreground,
    minHeight: 60,
    textAlignVertical: "top",
    marginBottom: 8,
  },
  popoverFooter: {
    flexDirection: "row",
    justifyContent: "flex-end",
  },
  applyButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    backgroundColor: theme.colors.accent,
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderRadius: 6,
  },
  applyButtonText: {
    color: "#ffffff",
    fontSize: 11,
    fontWeight: "600",
  },
}));
