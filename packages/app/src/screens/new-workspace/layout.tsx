import type { ReactNode } from "react";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
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
  children: ReactNode;
}

export function NewWorkspaceLayout({
  isCompact,
  fullscreen,
  title,
  formStack,
  onImportSession,
  children,
}: NewWorkspaceLayoutProps) {
  // In the header on compact layouts, under the composer otherwise.
  const importSessionButton = <ImportSessionButton compact={isCompact} onPress={onImportSession} />;
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
}));
