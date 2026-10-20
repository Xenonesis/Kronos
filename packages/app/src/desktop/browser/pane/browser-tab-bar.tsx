import React, { useCallback, useMemo } from "react";
import { Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Globe, Plus, X } from "lucide-react-native";
import { parseHostFromUrl } from "./browser-load-failure-overlay";
import { PressHighlight } from "@/components/ui/press-highlight";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import type { Theme } from "@/styles/theme";

const ThemedGlobe = withUnistyles(Globe);
const ThemedPlus = withUnistyles(Plus);
const ThemedX = withUnistyles(X);

const foregroundIconMapping = (theme: Theme) => ({ color: theme.colors.foreground });
const mutedIconMapping = (theme: Theme) => ({ color: theme.colors.foregroundMuted });

export interface BrowserTabItem {
  id: string;
  url: string;
  title?: string;
  isLoading?: boolean;
}

export interface BrowserTabBarProps {
  tabs: BrowserTabItem[];
  activeTabId: string;
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
  onNewTab: () => void;
  testID?: string;
}

function BrowserTab({
  tab,
  isActive,
  canClose,
  onSelectTab,
  onCloseTab,
}: {
  tab: BrowserTabItem;
  isActive: boolean;
  canClose: boolean;
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
}) {
  const displayLabel = tab.title?.trim() || parseHostFromUrl(tab.url) || "New Tab";

  const handleSelect = useCallback(() => {
    onSelectTab(tab.id);
  }, [onSelectTab, tab.id]);

  const handleClose = useCallback(
    (e: { stopPropagation: () => void }) => {
      e.stopPropagation();
      onCloseTab(tab.id);
    },
    [onCloseTab, tab.id],
  );

  const accessibilityState = useMemo(() => ({ selected: isActive }), [isActive]);

  return (
    <PressHighlight
      accessibilityRole="button"
      accessibilityState={accessibilityState}
      style={[styles.tab, isActive ? styles.tabActive : styles.tabInactive]}
      onPress={handleSelect}
      testID={`browser-tab-${tab.id}`}
    >
      <ThemedGlobe
        size={13}
        uniProps={isActive ? foregroundIconMapping : mutedIconMapping}
        style={styles.tabIcon}
      />
      <Text
        style={[styles.tabTitle, isActive ? styles.tabTitleActive : styles.tabTitleInactive]}
        numberOfLines={1}
      >
        {displayLabel}
      </Text>
      {canClose ? (
        <ToolbarButton
          label="Close Tab"
          tooltip="Close Tab"
          onPress={handleClose}
          style={styles.closeBtn}
          testID={`browser-tab-close-${tab.id}`}
        >
          <ThemedX size={12} uniProps={mutedIconMapping} />
        </ToolbarButton>
      ) : null}
    </PressHighlight>
  );
}

export function BrowserTabBar({
  tabs,
  activeTabId,
  onSelectTab,
  onCloseTab,
  onNewTab,
  testID = "browser-tab-bar",
}: BrowserTabBarProps) {
  return (
    <View style={styles.container} testID={testID}>
      <View style={styles.tabsList}>
        {tabs.map((tab) => (
          <BrowserTab
            key={tab.id}
            tab={tab}
            isActive={tab.id === activeTabId}
            canClose={tabs.length > 1}
            onSelectTab={onSelectTab}
            onCloseTab={onCloseTab}
          />
        ))}

        {/* Plus button for new tab */}
        <ToolbarButton
          label="New Tab"
          tooltip="New Tab"
          onPress={onNewTab}
          style={styles.newTabBtn}
          testID="browser-new-tab-btn"
        >
          <ThemedPlus size={14} uniProps={mutedIconMapping} />
        </ToolbarButton>
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    paddingHorizontal: theme.spacing[2],
    height: 34,
  },
  tabsList: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    flex: 1,
  },
  tab: {
    flexDirection: "row",
    alignItems: "center",
    height: 28,
    maxWidth: 220,
    minWidth: 100,
    paddingHorizontal: theme.spacing[2],
    borderRadius: theme.borderRadius.sm,
    gap: 6,
    borderWidth: 1,
  },
  tabActive: {
    backgroundColor: theme.colors.surface2,
    borderColor: theme.colors.border,
  },
  tabInactive: {
    backgroundColor: "transparent",
    borderColor: "transparent",
  },
  tabIcon: {
    flexShrink: 0,
  },
  tabTitle: {
    fontSize: theme.fontSize.sm,
    flex: 1,
  },
  tabTitleActive: {
    color: theme.colors.foreground,
    fontWeight: theme.fontWeight.medium,
  },
  tabTitleInactive: {
    color: theme.colors.foregroundMuted,
  },
  closeBtn: {
    width: 16,
    height: 16,
    borderRadius: 8,
    alignItems: "center",
    justifyContent: "center",
    marginLeft: 2,
  },
  newTabBtn: {
    width: 26,
    height: 26,
    borderRadius: theme.borderRadius.sm,
    alignItems: "center",
    justifyContent: "center",
    marginLeft: 2,
  },
}));
