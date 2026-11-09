import { memo, useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import {
  Code,
  Copy,
  CornerUpLeft,
  Eye,
  MoreHorizontal,
  Pin,
  PinOff,
  Volume2,
  VolumeX,
} from "lucide-react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ICON_SIZE, type Theme } from "@/styles/theme";
import { useToast } from "@/contexts/toast-context";
import { writeMarkdownToRichClipboard } from "@/utils/rich-clipboard";
import { getDefaultMarkdownClipboardEnvironment } from "@/utils/rich-clipboard-default-environment";
import { insertQuoteIntoDraft } from "@/utils/message-reply";
import {
  isSpeechSupported,
  speakMessage,
  stopSpeaking,
  useSpeechStore,
} from "@/utils/speech-synthesis";
import { usePinnedMessagesStore } from "@/stores/pinned-messages-store";
import { useAssistantMessageViewStore } from "@/stores/assistant-message-view-store";

export interface AssistantMessageActionsMenuProps {
  messageId: string;
  occurrenceKey?: string;
  getContent?: () => string;
  serverId?: string;
  agentId?: string;
  testID?: string;
}

const ThemedMoreHorizontal = withUnistyles(MoreHorizontal);
const ThemedReply = withUnistyles(CornerUpLeft);
const ThemedCopy = withUnistyles(Copy);
const ThemedVolume2 = withUnistyles(Volume2);
const ThemedVolumeX = withUnistyles(VolumeX);
const ThemedCode = withUnistyles(Code);
const ThemedEye = withUnistyles(Eye);
const ThemedPin = withUnistyles(Pin);
const ThemedPinOff = withUnistyles(PinOff);

const foregroundColorMapping = (theme: Theme) => ({ color: theme.colors.foreground });
const foregroundMutedColorMapping = (theme: Theme) => ({ color: theme.colors.foregroundMuted });

export const AssistantMessageActionsMenu = memo(function AssistantMessageActionsMenu({
  messageId,
  occurrenceKey,
  getContent,
  serverId,
  agentId,
  testID = "assistant-message-actions-menu",
}: AssistantMessageActionsMenuProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const [isOpen, setIsOpen] = useState(false);

  const isSpeaking = useSpeechStore((state) => state.speakingMessageId === messageId);
  const speechSupported = useMemo(() => isSpeechSupported(), []);

  const viewTargetId = occurrenceKey || messageId;
  const isRaw = useAssistantMessageViewStore(
    (state) => state.isRaw(messageId) || (occurrenceKey ? state.isRaw(occurrenceKey) : false),
  );
  const toggleRaw = useAssistantMessageViewStore((state) => state.toggleRaw);

  const pinScope = serverId && agentId ? `${serverId}:${agentId}` : "default";
  const isPinned = usePinnedMessagesStore(
    (state) =>
      state.isPinned(pinScope, messageId) ||
      (occurrenceKey ? state.isPinned(pinScope, occurrenceKey) : false),
  );
  const togglePin = usePinnedMessagesStore((state) => state.togglePin);

  const handleOpenChange = useCallback((next: boolean) => {
    setIsOpen(next);
  }, []);

  const handleReply = useCallback(() => {
    const content = getContent?.() ?? "";
    insertQuoteIntoDraft({
      serverId,
      agentId,
      quoteText: content,
    });
    setIsOpen(false);
  }, [agentId, getContent, serverId]);

  const handleCopyMessage = useCallback(async () => {
    const content = getContent?.() ?? "";
    if (!content) return;
    await writeMarkdownToRichClipboard(content, getDefaultMarkdownClipboardEnvironment());
    toast.copied(t("message.actions.copied"));
    setIsOpen(false);
  }, [getContent, t, toast]);

  const handleReadAloud = useCallback(() => {
    const content = getContent?.() ?? "";
    if (isSpeaking) {
      stopSpeaking();
    } else {
      speakMessage(messageId, content);
    }
    setIsOpen(false);
  }, [getContent, isSpeaking, messageId]);

  const handleToggleRaw = useCallback(() => {
    toggleRaw(viewTargetId);
    setIsOpen(false);
  }, [toggleRaw, viewTargetId]);

  const handleTogglePin = useCallback(() => {
    const nowPinned = togglePin(pinScope, viewTargetId);
    toast.show(
      nowPinned ? t("message.actions.messagePinned") : t("message.actions.messageUnpinned"),
      { variant: "success" },
    );
    setIsOpen(false);
  }, [pinScope, t, toast, togglePin, viewTargetId]);

  const handleCopyId = useCallback(async () => {
    const idToCopy = messageId || occurrenceKey || "";
    if (!idToCopy) return;
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      await navigator.clipboard.writeText(idToCopy);
    }
    toast.copied(t("message.actions.messageIdCopied"));
    setIsOpen(false);
  }, [messageId, occurrenceKey, t, toast]);

  const tooltipContent = useMemo(
    () => (
      <TooltipContent side="top" align="center" offset={8}>
        <Text style={styles.tooltipText}>{t("message.actions.moreOptions")}</Text>
      </TooltipContent>
    ),
    [t],
  );

  return (
    <DropdownMenu open={isOpen} onOpenChange={handleOpenChange}>
      <Tooltip delayDuration={250} enabledOnDesktop enabledOnMobile={false}>
        <TooltipTrigger asChild>
          <View style={styles.triggerSlot} collapsable={false}>
            <DropdownMenuTrigger
              accessibilityLabel={t("message.actions.moreOptions")}
              accessibilityRole="button"
              style={styles.trigger}
              testID={`${testID}-trigger`}
            >
              {({ hovered, open }) => (
                <ThemedMoreHorizontal
                  size={ICON_SIZE.sm}
                  uniProps={hovered || open ? foregroundColorMapping : foregroundMutedColorMapping}
                />
              )}
            </DropdownMenuTrigger>
          </View>
        </TooltipTrigger>
        {tooltipContent}
      </Tooltip>
      <DropdownMenuContent align="start" minWidth={210} side="bottom" testID={`${testID}-content`}>
        <DropdownMenuItem
          leading={<ThemedReply size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />}
          onSelect={handleReply}
          testID={`${testID}-reply`}
        >
          {t("message.actions.reply")}
        </DropdownMenuItem>
        <DropdownMenuItem
          leading={<ThemedCopy size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />}
          onSelect={handleCopyMessage}
          testID={`${testID}-copy-message`}
        >
          {t("message.actions.copyMessage")}
        </DropdownMenuItem>
        {speechSupported ? (
          <DropdownMenuItem
            leading={
              isSpeaking ? (
                <ThemedVolumeX size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />
              ) : (
                <ThemedVolume2 size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />
              )
            }
            onSelect={handleReadAloud}
            testID={`${testID}-read-aloud`}
          >
            {isSpeaking ? t("message.actions.stopReading") : t("message.actions.readAloud")}
          </DropdownMenuItem>
        ) : null}
        <DropdownMenuItem
          leading={
            isRaw ? (
              <ThemedEye size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />
            ) : (
              <ThemedCode size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />
            )
          }
          onSelect={handleToggleRaw}
          testID={`${testID}-raw-markdown`}
        >
          {isRaw ? t("message.actions.showRenderedMarkdown") : t("message.actions.showRawMarkdown")}
        </DropdownMenuItem>
        <DropdownMenuItem
          leading={
            isPinned ? (
              <ThemedPinOff size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />
            ) : (
              <ThemedPin size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />
            )
          }
          onSelect={handleTogglePin}
          testID={`${testID}-pin`}
        >
          {isPinned ? t("message.actions.unpinMessage") : t("message.actions.pinMessage")}
        </DropdownMenuItem>
        <DropdownMenuItem
          leading={<ThemedCopy size={ICON_SIZE.sm} uniProps={foregroundColorMapping} />}
          onSelect={handleCopyId}
          testID={`${testID}-copy-id`}
        >
          {t("message.actions.copyMessageId")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
});

const styles = StyleSheet.create((theme) => ({
  trigger: {
    padding: theme.spacing[1],
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "transparent",
  },
  triggerSlot: {
    alignSelf: "center",
  },
  tooltipText: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.sm,
  },
}));
