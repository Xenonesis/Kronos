import { useHosts, useHostRuntimeLastError } from "@/runtime/host-runtime";
import { useCallback, useEffect, useState, type ComponentType } from "react";
import { useTranslation } from "react-i18next";
import { View, Text, Pressable, ScrollView, TextInput } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { useRouter } from "expo-router";
import { FolderOpen, Inbox, Plug, Smartphone, MessageSquarePlus, Sparkles } from "lucide-react-native";
import { PaseoLogo } from "@/components/icons/paseo-logo";
import { CommunityLinks } from "@/components/community-links";
import { MenuHeader } from "@/components/headers/menu-header";
import { useOpenAddProject } from "@/hooks/use-open-add-project";
import { useImportSession } from "@/hooks/use-import-session";
import { useHostChooser } from "@/hosts/host-chooser";
import { usePanelStore } from "@/stores/panel-store";
import {
  useIsCompactFormFactor,
  HEADER_INNER_HEIGHT,
  HEADER_INNER_HEIGHT_MOBILE,
  HEADER_TOP_PADDING_MOBILE,
} from "@/constants/layout";
import { useLocalDaemonServerId } from "@/hooks/use-is-local-daemon";
import { PairDeviceModal } from "@/desktop/components/pair-device-modal";
import { buildSettingsHostSectionRoute, buildNewWorkspaceRoute } from "@/utils/host-routes";
import { generateDraftId, buildNewWorkspaceDraftKey } from "@/stores/draft-keys";
import { useDraftStore } from "@/stores/draft-store";

const WINDOW_CONTENT_DATASET = { "window-content": "" };

export function OpenProjectScreen() {
  const { t } = useTranslation();
  const { theme } = useUnistyles();
  const hosts = useHosts();
  const router = useRouter();
  const openDesktopAgentList = usePanelStore((s) => s.openDesktopAgentList);
  const openProjectPicker = useOpenAddProject();
  const importSession = useImportSession();
  const chooseHost = useHostChooser();
  const localServerId = useLocalDaemonServerId();
  const [isPairDeviceOpen, setIsPairDeviceOpen] = useState(false);
  const [quickPrompt, setQuickPrompt] = useState("");

  const isCompactLayout = useIsCompactFormFactor();

  useEffect(() => {
    if (!isCompactLayout) {
      openDesktopAgentList();
    }
  }, [isCompactLayout, openDesktopAgentList]);

  const handleOpenPicker = useCallback(() => {
    void openProjectPicker();
  }, [openProjectPicker]);

  const handleOpenPairDevice = useCallback(() => setIsPairDeviceOpen(true), []);
  const handleClosePairDevice = useCallback(() => setIsPairDeviceOpen(false), []);

  const handleOpenProviders = useCallback(() => {
    chooseHost({
      title: "Choose host",
      onChooseHost: (serverId) => {
        router.push(buildSettingsHostSectionRoute(serverId, "providers"));
      },
    });
  }, [chooseHost, router]);

  const handleOpenInstantChat = useCallback(
    (customText?: string) => {
      const serverId = localServerId ?? hosts[0]?.serverId;
      if (!serverId) return;
      const textToUse = typeof customText === "string" ? customText.trim() : quickPrompt.trim();
      const draftId = generateDraftId();
      if (textToUse) {
        const draftKey = buildNewWorkspaceDraftKey(draftId);
        useDraftStore.getState().saveDraftInput({
          draftKey,
          draft: { text: textToUse, attachments: [] },
        });
      }
      router.push(
        buildNewWorkspaceRoute({
          serverId,
          projectId: "prj_scratchpad",
          displayName: "General Chat",
          draftId,
        }),
      );
    },
    [hosts, localServerId, quickPrompt, router],
  );

  return (
    <View style={styles.container}>
      <MenuHeader borderless />
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={styles.scrollContent}
        dataSet={WINDOW_CONTENT_DATASET}
      >
        <View style={styles.content}>
          <View style={styles.logo}>
            <PaseoLogo size={52} />
          </View>
          {hosts.map((host) => (
            <HostError key={host.serverId} serverId={host.serverId} label={host.label} />
          ))}

          <View style={styles.quickChatBox}>
            <Text style={styles.quickChatTitle}>
              {t("openProject.quickChat.title", "What should we work on?")}
            </Text>
            <View style={styles.quickChatInputWrapper}>
              <TextInput
                style={styles.quickChatInput}
                placeholder={t(
                  "openProject.quickChat.placeholder",
                  "Ask any question or test an agent (no project needed)...",
                )}
                placeholderTextColor={theme.colors.foregroundMuted}
                value={quickPrompt}
                onChangeText={setQuickPrompt}
                onSubmitEditing={() => handleOpenInstantChat()}
                returnKeyType="go"
              />
              <Pressable
                style={styles.quickChatSendButton}
                onPress={() => handleOpenInstantChat()}
                accessibilityLabel="Start Chat"
                testID="open-project-quick-chat-send"
              >
                <Sparkles size={18} color={theme.colors.accent} />
              </Pressable>
            </View>
          </View>

          <View style={styles.tiles}>
            <HomeTile
              icon={MessageSquarePlus}
              title={t("openProject.tiles.instantChat.title", "Instant Chat")}
              description={t(
                "openProject.tiles.instantChat.description",
                "Chat immediately with any agent without adding a project.",
              )}
              onPress={() => handleOpenInstantChat()}
              testID="open-project-instant-chat"
              accent
            />
            <HomeTile
              icon={FolderOpen}
              title={t("openProject.tiles.addProject.title")}
              description={t("openProject.tiles.addProject.description")}
              onPress={handleOpenPicker}
              testID="open-project-submit"
            />
            <HomeTile
              icon={Inbox}
              title={t("openProject.tiles.importSession.title")}
              description={t("openProject.tiles.importSession.description")}
              onPress={importSession.open}
              testID="open-project-import-session"
            />
            <HomeTile
              icon={Plug}
              title={t("openProject.tiles.setupProviders.title")}
              description={t("openProject.tiles.setupProviders.description")}
              onPress={handleOpenProviders}
              testID="open-project-setup-providers"
            />
            {localServerId ? (
              <HomeTile
                icon={Smartphone}
                title={t("openProject.tiles.pairDevice.title")}
                description={t("openProject.tiles.pairDevice.description")}
                onPress={handleOpenPairDevice}
                testID="open-project-pair-device"
              />
            ) : null}
          </View>
        </View>
        <View style={styles.communityRow}>
          <CommunityLinks />
        </View>
      </ScrollView>
      <PairDeviceModal
        serverId={localServerId ?? ""}
        visible={isPairDeviceOpen}
        onClose={handleClosePairDevice}
        testID="open-project-pair-device-modal"
      />
      {importSession.sheet}
    </View>
  );
}

function HostError({ serverId, label }: { serverId: string; label: string }) {
  const error = useHostRuntimeLastError(serverId);
  return error ? (
    <Text accessibilityRole="alert" style={styles.hostError}>
      {label}: {error}
    </Text>
  ) : null;
}

interface HomeTileProps {
  icon: ComponentType<{ size: number; color: string }>;
  title: string;
  description: string;
  onPress: () => void;
  testID?: string;
  accent?: boolean;
}

function HomeTile({ icon: Icon, title, description, onPress, testID, accent }: HomeTileProps) {
  // useUnistyles is acceptable here: leaf component, off the hot path (home screen renders once).
  const { theme } = useUnistyles();
  const [hovered, setHovered] = useState(false);
  const handleHoverIn = useCallback(() => setHovered(true), []);
  const handleHoverOut = useCallback(() => setHovered(false), []);

  const iconColor = accent ? theme.colors.accent : theme.colors.foregroundMuted;

  const pressableStyle = useCallback(
    ({ pressed }: { pressed: boolean }) => [
      styles.tile,
      hovered && styles.tileHovered,
      pressed && styles.tilePressed,
    ],
    [hovered],
  );

  return (
    <Pressable
      onPress={onPress}
      onHoverIn={handleHoverIn}
      onHoverOut={handleHoverOut}
      testID={testID}
      style={pressableStyle}
    >
      <Icon size={20} color={iconColor} />
      <View style={styles.tileText}>
        <Text style={styles.tileTitle}>{title}</Text>
        <Text style={styles.tileDescription}>{description}</Text>
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    userSelect: "none",
  },
  scroll: {
    flex: 1,
  },
  scrollContent: {
    flexGrow: 1,
  },
  content: {
    position: "relative",
    flexGrow: 1,
    flexShrink: 0,
    justifyContent: { xs: "flex-start", md: "center" },
    alignItems: "center",
    gap: 0,
    padding: theme.spacing[6],
    paddingTop: { xs: theme.spacing[12], md: theme.spacing[6] },
  },
  logo: {
    marginBottom: theme.spacing[6],
  },
  quickChatBox: {
    width: "100%",
    maxWidth: 452,
    marginBottom: theme.spacing[4],
    gap: theme.spacing[2],
  },
  quickChatTitle: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.semibold,
    color: theme.colors.foreground,
    textAlign: "left",
  },
  quickChatInputWrapper: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: theme.colors.surface1,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.xl,
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[1],
  },
  quickChatInput: {
    flex: 1,
    fontSize: theme.fontSize.base,
    color: theme.colors.foreground,
    paddingVertical: theme.spacing[2],
    paddingRight: theme.spacing[2],
  },
  quickChatSendButton: {
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.lg,
    backgroundColor: theme.colors.surface2,
    justifyContent: "center",
    alignItems: "center",
  },
  hostError: {
    color: theme.colors.destructive,
    fontSize: theme.fontSize.base,
    maxWidth: 452,
    textAlign: "center",
  },
  tiles: {
    marginTop: { xs: theme.spacing[6], md: theme.spacing[12] },
    width: "100%",
    maxWidth: 452,
    flexDirection: "row",
    flexWrap: "wrap",
    justifyContent: "flex-start",
    gap: theme.spacing[3],
  },
  tile: {
    width: { xs: "100%", md: 220 },
    minHeight: { xs: 0, md: 132 },
    padding: theme.spacing[4],
    backgroundColor: theme.colors.surface1,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.xl,
    gap: theme.spacing[3],
  },
  tileHovered: {
    backgroundColor: theme.colors.surface2,
    borderColor: theme.colors.borderAccent,
  },
  tilePressed: {
    opacity: 0.85,
  },
  tileText: {
    gap: theme.spacing[1],
  },
  tileTitle: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.normal,
  },
  tileDescription: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
    lineHeight: 18,
  },
  communityRow: {
    flexShrink: 0,
    paddingBottom: {
      xs: HEADER_INNER_HEIGHT_MOBILE + HEADER_TOP_PADDING_MOBILE + theme.spacing[2],
      md: HEADER_INNER_HEIGHT + theme.spacing[2],
    },
    flexDirection: "row",
    justifyContent: "center",
    alignItems: "center",
    gap: 0,
  },
}));
