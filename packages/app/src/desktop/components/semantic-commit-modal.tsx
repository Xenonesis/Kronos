import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { GitCommitHorizontal, Sparkles, X, Check, FileCode } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { PressHighlight } from "@/components/ui/press-highlight";
import { invokeTauri } from "@/utils/tauri-invoke";
import { useToast } from "@/contexts/toast-context";

const ThemedCommitIcon = withUnistyles(GitCommitHorizontal, (theme) => ({
  color: theme.colors.accent,
}));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedSparklesIcon = withUnistyles(Sparkles, (theme) => ({ color: theme.colors.accent }));
const ThemedFileCodeIcon = withUnistyles(FileCode, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

const ChangedFilesList = memo(function ChangedFilesList({
  filesChanged,
}: {
  filesChanged: string[];
}) {
  if (filesChanged.length === 0) return null;
  return (
    <View style={styles.filesBox}>
      <Text style={styles.label}>Files to Stage ({filesChanged.length})</Text>
      {filesChanged.slice(0, 12).map((f) => (
        <View key={f} style={styles.fileRow}>
          <ThemedFileCodeIcon size={12} />
          <Text style={styles.fileName} numberOfLines={1}>
            {f}
          </Text>
        </View>
      ))}
    </View>
  );
});

const CommitEditorBody = memo(function CommitEditorBody({
  errorText,
  message,
  messageResetKey,
  filesChanged,
  onChangeMessage,
  onRegenerate,
}: {
  errorText: string | null;
  message: string;
  messageResetKey: number;
  filesChanged: string[];
  onChangeMessage: (text: string) => void;
  onRegenerate: () => void;
}) {
  if (errorText) {
    return (
      <ScrollView style={styles.body}>
        <View style={styles.errorBox}>
          <Text style={styles.errorText}>{errorText}</Text>
        </View>
      </ScrollView>
    );
  }
  return (
    <ScrollView style={styles.body}>
      <View style={styles.labelRow}>
        <Text style={styles.label}>Conventional Commit Message</Text>
        <PressHighlight onPress={onRegenerate} style={styles.regenBtn}>
          <ThemedSparklesIcon size={13} />
          <Text style={styles.regenText}>Regenerate</Text>
        </PressHighlight>
      </View>

      <AdaptiveTextInput
        initialValue={message}
        resetKey={messageResetKey}
        onChangeText={onChangeMessage}
        multiline
        numberOfLines={6}
        style={styles.textarea}
      />

      <ChangedFilesList filesChanged={filesChanged} />
    </ScrollView>
  );
});

interface SemanticCommitSuggestion {
  commitType: string;
  scope?: string | null;
  subject: string;
  body: string;
  fullMessage: string;
  filesChanged: string[];
}

export interface SemanticCommitModalProps {
  visible: boolean;
  cwd: string;
  onClose: () => void;
}

export const SemanticCommitModal = memo(function SemanticCommitModal({
  visible,
  cwd,
  onClose,
}: SemanticCommitModalProps) {
  const toast = useToast();
  const [message, setMessage] = useState("");
  const [messageResetKey, setMessageResetKey] = useState(0);
  const [filesChanged, setFilesChanged] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [errorText, setErrorText] = useState<string | null>(null);

  const loadSuggestion = useCallback(async () => {
    setIsLoading(true);
    setErrorText(null);
    try {
      const [diffSuggestion, aiMessage] = await Promise.all([
        invokeTauri<SemanticCommitSuggestion>("suggest_semantic_commit", {
          repoPath: cwd,
        }).catch(() => null),
        invokeTauri<string>("generate_ai_semantic_commit", {
          cwd,
        }).catch(() => null),
      ]);

      if (aiMessage?.trim()) {
        setMessage(aiMessage.trim());
        setFilesChanged(diffSuggestion?.filesChanged ?? []);
      } else if (diffSuggestion) {
        setMessage(diffSuggestion.fullMessage);
        setFilesChanged(diffSuggestion.filesChanged);
      } else {
        setErrorText("No staged/unstaged changes found or git status unavailable.");
        setMessage("");
        setFilesChanged([]);
      }
      setMessageResetKey((k) => k + 1);
    } catch (err) {
      setErrorText(String(err));
    } finally {
      setIsLoading(false);
    }
  }, [cwd]);

  const handleRegeneratePress = useCallback(() => {
    void loadSuggestion();
  }, [loadSuggestion]);

  useEffect(() => {
    if (visible) {
      void loadSuggestion();
    }
  }, [visible, loadSuggestion]);

  const handleCommit = useCallback(async () => {
    if (!message.trim()) return;
    setIsLoading(true);
    try {
      const res = await invokeTauri<string>("execute_semantic_commit", {
        repoPath: cwd,
        message: message.trim(),
      });
      toast.show(res ? `Committed: ${res.split("\n")[0]}` : "Semantic commit created", {
        variant: "success",
      });
      onClose();
    } catch (err) {
      toast.show(`Commit failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsLoading(false);
    }
  }, [cwd, message, onClose, toast]);

  const handleCommitPress = useCallback(() => {
    void handleCommit();
  }, [handleCommit]);

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop} testID="semantic-commit-modal">
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedCommitIcon size={18} />
              <Text style={styles.title}>AI Semantic Git Commit Generator</Text>
            </View>
            <ToolbarButton label="Close" onPress={onClose}>
              <ThemedCloseIcon size={18} />
            </ToolbarButton>
          </View>

          <CommitEditorBody
            errorText={errorText}
            message={message}
            messageResetKey={messageResetKey}
            filesChanged={filesChanged}
            onChangeMessage={setMessage}
            onRegenerate={handleRegeneratePress}
          />

          <View style={styles.footer}>
            <Button size="sm" variant="outline" onPress={onClose}>
              <Text style={styles.cancelText}>Cancel</Text>
            </Button>
            <Button
              size="sm"
              onPress={handleCommitPress}
              disabled={isLoading || Boolean(errorText) || !message.trim()}
            >
              <Check size={14} color="#ffffff" />
              <Text style={styles.commitText}>Stage All & Commit</Text>
            </Button>
          </View>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.65)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  card: {
    width: "100%",
    maxWidth: 620,
    maxHeight: "82%",
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    padding: theme.spacing[4],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  titleRow: { flexDirection: "row", alignItems: "center", gap: 8 },
  title: { fontSize: theme.fontSize.base, fontWeight: "700", color: theme.colors.foreground },
  closeBtn: { padding: 4 },
  body: { padding: theme.spacing[4] },
  errorBox: { padding: 14, borderRadius: 8, backgroundColor: "rgba(239,68,68,0.12)" },
  errorText: { color: "#ef4444", fontSize: 13 },
  labelRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: 8,
  },
  label: { fontSize: 12, fontWeight: "600", color: theme.colors.foreground },
  regenBtn: { flexDirection: "row", alignItems: "center", gap: 4 },
  regenText: { fontSize: 12, color: theme.colors.accent, fontWeight: "600" },
  textarea: {
    minHeight: 130,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.md,
    padding: 10,
    color: theme.colors.foreground,
    fontFamily: "monospace",
    fontSize: 12,
    textAlignVertical: "top",
  },
  filesBox: { marginTop: 14, gap: 6 },
  fileRow: { flexDirection: "row", alignItems: "center", gap: 6 },
  fileName: { fontSize: 12, color: theme.colors.foregroundMuted, fontFamily: "monospace" },
  footer: {
    flexDirection: "row",
    justifyContent: "flex-end",
    gap: 10,
    padding: theme.spacing[4],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  cancelText: { color: theme.colors.foreground, fontSize: 12, fontWeight: "600" },
  commitText: { color: "#ffffff", fontSize: 12, fontWeight: "600", marginLeft: 4 },
}));
