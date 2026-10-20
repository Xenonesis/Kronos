import React, { useCallback, useMemo, useState } from "react";
import {
  type NativeSyntheticEvent,
  Text,
  TextInput,
  type TextInputKeyPressEventData,
  View,
} from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { AlertCircle, CornerDownLeft, MessageSquare } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import {
  type DiffAnnotation,
  formatDiffAnnotationPrompt,
  validateDiffAnnotation,
} from "./diff-comment-model";

export interface DiffCommentClient {
  send: (agentId: string, prompt: string) => Promise<unknown> | unknown;
}

export type CompatibleDiffCommentClient =
  | DiffCommentClient
  | {
      send?: (agentId: string, prompt: string) => Promise<unknown> | unknown;
      sendAgentMessage: (agentId: string, text: string) => Promise<unknown> | unknown;
    };

export interface DiffAnnotationInput {
  filePath: string;
  lineNumber: number;
  diffHunk?: string;
  originalLineText?: string;
  feedbackText?: string;
  side?: "old" | "new";
  id?: string;
  createdAt?: string;
}

export interface DiffCommentCardProps {
  annotation: DiffAnnotationInput;
  agentId: string;
  client: CompatibleDiffCommentClient;
  onCancel: () => void;
  onSuccess?: (sentPrompt: string) => void;
  onError?: (error: Error) => void;
  initialFeedback?: string;
  placeholder?: string;
  disabled?: boolean;
  autoFocus?: boolean;
  testID?: string;
}

/**
 * Compact inline card for authoring line-level comments on diffs
 * and dispatching formatted prompts directly to the active agent.
 */
export function DiffCommentCard({
  annotation,
  agentId,
  client,
  onCancel,
  onSuccess,
  onError,
  initialFeedback,
  placeholder,
  disabled = false,
  autoFocus = true,
  testID = "diff-comment-card",
}: DiffCommentCardProps) {
  const [feedback, setFeedback] = useState(initialFeedback ?? annotation.feedbackText ?? "");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const trimmed = feedback.trim();
  const canSubmit = !disabled && !isSubmitting && trimmed.length > 0;

  const handleTextChange = useCallback((text: string) => {
    setFeedback(text);
    setErrorMessage(null);
  }, []);

  const handleCancel = useCallback(() => {
    if (isSubmitting) return;
    onCancel();
  }, [isSubmitting, onCancel]);

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return;

    const validation = validateDiffAnnotation({
      filePath: annotation.filePath,
      lineNumber: annotation.lineNumber,
      feedbackText: trimmed,
    });

    if (!validation.isValid) {
      const msg = validation.errors.map((e) => e.message).join(", ");
      setErrorMessage(msg);
      return;
    }

    setIsSubmitting(true);
    setErrorMessage(null);

    const fullAnnotation: DiffAnnotation = {
      filePath: annotation.filePath,
      lineNumber: annotation.lineNumber,
      diffHunk: annotation.diffHunk ?? "",
      originalLineText: annotation.originalLineText ?? "",
      feedbackText: trimmed,
      side: annotation.side,
      id: annotation.id,
      createdAt: annotation.createdAt,
    };

    const prompt = formatDiffAnnotationPrompt(fullAnnotation);

    try {
      if (typeof client.send === "function") {
        await client.send(agentId, prompt);
      } else if ("sendAgentMessage" in client && typeof client.sendAgentMessage === "function") {
        await client.sendAgentMessage(agentId, prompt);
      } else {
        throw new Error("Client does not provide a send method");
      }

      setIsSubmitting(false);
      onSuccess?.(prompt);
    } catch (err) {
      setIsSubmitting(false);
      const errObj = err instanceof Error ? err : new Error(String(err));
      setErrorMessage(errObj.message || "Failed to dispatch comment to agent");
      onError?.(errObj);
    }
  }, [
    agentId,
    annotation.createdAt,
    annotation.diffHunk,
    annotation.filePath,
    annotation.id,
    annotation.lineNumber,
    annotation.originalLineText,
    annotation.side,
    canSubmit,
    client,
    onError,
    onSuccess,
    trimmed,
  ]);

  const handleKeyPress = useCallback(
    (e: NativeSyntheticEvent<TextInputKeyPressEventData>) => {
      const nativeEvent = e.nativeEvent as TextInputKeyPressEventData & {
        ctrlKey?: boolean;
        metaKey?: boolean;
        shiftKey?: boolean;
      };
      if (nativeEvent.key === "Escape") {
        handleCancel();
      } else if (nativeEvent.key === "Enter" && (nativeEvent.ctrlKey || nativeEvent.metaKey)) {
        void handleSubmit();
      }
    },
    [handleCancel, handleSubmit],
  );

  const handleWebKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        handleCancel();
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        void handleSubmit();
      }
    },
    [handleCancel, handleSubmit],
  );

  const headerLabel = `${annotation.filePath}:${annotation.lineNumber}`;
  const sideLabel = annotation.side === "old" ? "base" : annotation.side === "new" ? "head" : null;

  return (
    <View style={styles.card} testID={testID}>
      <View style={styles.header}>
        <View style={styles.headerLeft}>
          <MessageSquare size={13} color="#888" />
          <Text style={styles.headerText} numberOfLines={1}>
            {headerLabel}
          </Text>
          {sideLabel ? (
            <View
              style={[
                styles.sideBadge,
                annotation.side === "old" ? styles.sideBadgeOld : styles.sideBadgeNew,
              ]}
            >
              <Text style={styles.sideBadgeText}>{sideLabel}</Text>
            </View>
          ) : null}
        </View>
      </View>

      {annotation.originalLineText && annotation.originalLineText.trim().length > 0 ? (
        <View style={styles.codePreview}>
          <Text style={styles.codePreviewText} numberOfLines={2}>
            {annotation.originalLineText}
          </Text>
        </View>
      ) : null}

      <TextInput
        style={styles.textInput}
        multiline
        placeholder={placeholder ?? "Leave a comment for the agent... (Ctrl+Enter to send)"}
        placeholderTextColor={styles.placeholder.color}
        value={feedback}
        onChangeText={handleTextChange}
        editable={!disabled && !isSubmitting}
        autoFocus={autoFocus}
        onKeyPress={handleKeyPress}
        // @ts-expect-error onKeyDown is supported on React Native Web
        onKeyDown={handleWebKeyDown}
        testID={`${testID}-input`}
      />

      {errorMessage ? (
        <View style={styles.errorBanner} testID={`${testID}-error`}>
          <AlertCircle size={13} color="#ef4444" />
          <Text style={styles.errorText} numberOfLines={2}>
            {errorMessage}
          </Text>
        </View>
      ) : null}

      <View style={styles.footer}>
        <View style={styles.hintContainer}>
          <CornerDownLeft size={11} color="#aaa" />
          <Text style={styles.hintText}>Ctrl+Enter to send</Text>
        </View>
        <View style={styles.actionButtons}>
          <Button
            variant="ghost"
            size="xs"
            onPress={handleCancel}
            disabled={isSubmitting}
            testID={`${testID}-cancel-btn`}
          >
            Cancel
          </Button>
          <Button
            variant="default"
            size="xs"
            onPress={handleSubmit}
            disabled={!canSubmit}
            loading={isSubmitting}
            testID={`${testID}-send-btn`}
          >
            Send to Agent
          </Button>
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  card: {
    borderWidth: 1,
    borderColor: theme.colors.border,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    padding: theme.spacing[2],
    gap: theme.spacing[2],
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  headerLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
    flexShrink: 1,
  },
  headerText: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.medium,
    color: theme.colors.foreground,
  },
  sideBadge: {
    paddingHorizontal: 5,
    paddingVertical: 1,
    borderRadius: theme.borderRadius.sm,
  },
  sideBadgeOld: {
    backgroundColor: theme.colors.surface3,
  },
  sideBadgeNew: {
    backgroundColor: theme.colors.surface3,
  },
  sideBadgeText: {
    fontSize: 10,
    color: theme.colors.foregroundMuted,
    fontWeight: theme.fontWeight.medium,
  },
  codePreview: {
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.sm,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: theme.spacing[1],
    borderLeftWidth: 2,
    borderLeftColor: theme.colors.accent,
  },
  codePreviewText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontFamily: "monospace",
  },
  textInput: {
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.md,
    padding: theme.spacing[2],
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    minHeight: 64,
    textAlignVertical: "top",
  },
  placeholder: {
    color: theme.colors.foregroundMuted,
  },
  errorBanner: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: theme.spacing[1],
    borderRadius: theme.borderRadius.sm,
  },
  errorText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.destructive,
    flexShrink: 1,
  },
  footer: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  hintContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
  },
  hintText: {
    fontSize: 10,
    color: theme.colors.foregroundMuted,
  },
  actionButtons: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
}));
