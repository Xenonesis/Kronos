import type { ReactNode } from "react";
import { Text, View } from "react-native";
import { Layers } from "lucide-react-native";
import { StyleSheet } from "react-native-unistyles";
import { Button } from "@/components/ui/button";
import { ComposerDock } from "@/composer/dock";
import { TitlebarDragRegion } from "@/components/desktop/titlebar-drag-region";
import { MenuHeader } from "@/components/headers/menu-header";
import { ImportSessionButton } from "./import-session-button";

export interface NewWorkspaceLayoutProps {
  isCompact: boolean;
  /** The composer covers the screen; the header leaves and the dock fills the content. */
  fullscreen: boolean;
  title: string;
  formStack: ReactNode;
  onImportSession: () => void;
  onOpenTemplates?: () => void;
  children: ReactNode;
}

export function NewWorkspaceLayout({
  isCompact,
  fullscreen,
  title,
  formStack,
  onImportSession,
  onOpenTemplates,
  children,
}: NewWorkspaceLayoutProps) {
  // In the header on compact layouts, under the composer otherwise.
  const importSessionButton = (
    <View style={isCompact ? styles.compactActions : styles.wideActions}>
      <ImportSessionButton compact={isCompact} onPress={onImportSession} />
      {onOpenTemplates ? (
        <View style={isCompact ? null : styles.templateButtonWrap}>
          <Button
            variant="ghost"
            leftIcon={Layers}
            onPress={onOpenTemplates}
            style={isCompact ? null : styles.pill}
            testID="new-workspace-open-templates"
          >
            From Template
          </Button>
        </View>
      ) : null}
    </View>
  );
  // Compact has no title: the fields name themselves. The free space above pushes them down onto
  // the composer, where the thumb is.
  const setupFields = isCompact ? (
    <>
      <View style={styles.compactSetupSpace} pointerEvents="none" />
      {formStack}
    </>
  ) : (
    <>
      <View style={styles.composerTitleContainer} pointerEvents="none">
        <Text style={styles.composerTitle}>{title}</Text>
      </View>
      {formStack}
    </>
  );
  return (
    <>
      {fullscreen ? null : (
        <MenuHeader borderless rightContent={isCompact ? importSessionButton : undefined} />
      )}
      <View style={styles.content}>
        <TitlebarDragRegion />
        <ComposerDock centered={!isCompact} fullscreen={fullscreen}>
          {setupFields}
          <>
            {children}
            {isCompact || fullscreen ? null : importSessionButton}
          </>
        </ComposerDock>
      </View>
    </>
  );
}

const styles = StyleSheet.create((theme) => ({
  content: {
    position: "relative",
    flex: 1,
  },
  compactSetupSpace: {
    flex: 1,
  },
  composerTitleContainer: {
    marginBottom: theme.spacing[8],
    paddingLeft: theme.spacing[6],
    paddingRight: theme.spacing[4],
  },
  composerTitle: {
    fontSize: theme.fontSize["2xl"],
    fontWeight: theme.fontWeight.normal,
    color: theme.colors.foreground,
  },
  compactActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  wideActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    flexWrap: "wrap",
  },
  templateButtonWrap: {
    paddingTop: theme.spacing[4],
  },
  pill: {
    borderRadius: theme.borderRadius.full,
    borderColor: theme.colors.borderAccent,
  },
}));
