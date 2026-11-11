import { memo, useState, useCallback } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Sparkles, X, CornerDownLeft } from "lucide-react-native";
import { EditingTextInput } from "@/components/ui/text-input";
import { Button } from "@/components/ui/button";

const ThemedSparkles = withUnistyles(Sparkles);
const ThemedX = withUnistyles(X);
const ThemedCorner = withUnistyles(CornerDownLeft);

export interface FloatingInlineEditProps {
  filePath: string;
  selectedText?: string;
  lineStart?: number;
  lineEnd?: number;
  visible: boolean;
  onClose: () => void;
  onSubmit: (formattedPrompt: string) => void;
}

export const FloatingInlineEdit = memo(function FloatingInlineEdit({
  filePath,
  selectedText = "",
  lineStart,
  lineEnd,
  visible,
  onClose,
  onSubmit,
}: FloatingInlineEditProps) {
  const [prompt, setPrompt] = useState("");

  const handleSubmit = useCallback(() => {
    const trimmed = prompt.trim();
    if (!trimmed) return;

    const lineScope =
      lineStart !== undefined && lineEnd !== undefined ? ` (Lines ${lineStart}-${lineEnd})` : "";

    const formatted = `### ⚡ Targeted Inline Edit: \`${filePath}\`${lineScope}
${selectedText ? `\`\`\`\n${selectedText.trim()}\n\`\`\`\n` : ""}
**Request:** ${trimmed}
Please provide an exact, targeted patch for this section.`;

    onSubmit(formatted);
    setPrompt("");
    onClose();
  }, [prompt, filePath, lineStart, lineEnd, selectedText, onSubmit, onClose]);

  if (!visible) return null;

  return (
    <View style={styles.floatingContainer} testID="floating-inline-edit">
      <View style={styles.headerRow}>
        <View style={styles.headerLeft}>
          <ThemedSparkles size={14} color="#6366f1" />
          <Text style={styles.headerTitle}>
            Inline Edit: {filePath.split("/").pop()}
            {lineStart !== undefined ? `:${lineStart}` : ""}
          </Text>
        </View>
        <Button size="sm" variant="ghost" onPress={onClose} style={styles.closeBtn}>
          <ThemedX size={13} color="#888" />
        </Button>
      </View>

      <View style={styles.inputRow}>
        <EditingTextInput
          initialValue={prompt}
          onChangeText={setPrompt}
          placeholder="Ask Kronos to edit, refactor, or test this selection (Ctrl+K)..."
          onSubmitEditing={handleSubmit}
          autoFocus
          style={styles.textInput}
        />
        <Button size="sm" variant="default" onPress={handleSubmit} disabled={!prompt.trim()}>
          <ThemedCorner size={12} color="#fff" />
          <Text style={styles.sendText}>Edit</Text>
        </Button>
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  floatingContainer: {
    position: "absolute",
    bottom: theme.spacing[4],
    left: theme.spacing[4],
    right: theme.spacing[4],
    backgroundColor: theme.colors.surface0,
    borderColor: theme.colors.border,
    borderWidth: 1,
    borderRadius: theme.borderRadius.lg,
    padding: theme.spacing[3],
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.15,
    shadowRadius: 12,
    elevation: 8,
    gap: theme.spacing[1],
    zIndex: 999,
  },
  headerRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  headerLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  headerTitle: {
    fontSize: theme.fontSize.code,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  closeBtn: {
    padding: 2,
    minHeight: 20,
  },
  inputRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  textInput: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    backgroundColor: theme.colors.surface1 ?? "transparent",
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 6,
    borderRadius: theme.borderRadius.sm,
    color: theme.colors.foreground,
  },
  sendText: {
    fontSize: theme.fontSize.code,
    color: "#fff",
    fontWeight: "600",
  },
}));
