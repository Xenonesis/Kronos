import { useCallback, useMemo, useRef, useState, type ComponentType, type ReactNode } from "react";
import { Pressable, Text, View, type PressableStateCallbackType } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { useTranslation } from "react-i18next";
import { Plus, Server } from "lucide-react-native";
import { ComboboxTrigger } from "@/components/ui/combobox-trigger";
import { HostPicker as SharedHostPicker } from "@/components/hosts/host-picker";
import { HostStatusDot } from "@/components/host-status-dot";
import type { HostProfile } from "@/types/host-connection";
import type { EnableBuiltInDaemonOption } from "@/desktop/hooks/use-enable-built-in-daemon-option";
import type { HostSectionSlug, SettingsSectionSlug } from "@/utils/host-routes";
import { densityIconBox } from "@/styles/density";

export interface SidebarSectionItem {
  id: SettingsSectionSlug;
  labelKey: string;
  icon: ComponentType<{ size: number; color: string; strokeWidth?: number }>;
  desktopOnly?: boolean;
  webOnly?: boolean;
  /** The page body, for pages that need nothing from the settings screen. */
  Content?: ComponentType;
}

export interface HostSectionItem {
  id: HostSectionSlug;
  labelKey: string;
  icon: ComponentType<{ size: number; color: string; strokeWidth?: number }>;
}

interface SidebarAppearanceProps {
  iconSize: number;
  foreground: string;
  foregroundMuted: string;
}

interface SettingsSidebarContentProps extends SidebarAppearanceProps {
  items: SidebarSectionItem[];
  hostItems: HostSectionItem[];
  selectedSectionId: SettingsSectionSlug | null;
  selectedHostSection: HostSectionSlug | null;
  activeHostServerId: string | null;
  sortedHosts: HostProfile[];
  enableBuiltInDaemonOption: EnableBuiltInDaemonOption;
  onSelectSection: (section: SettingsSectionSlug) => void;
  onSelectHostSection: (section: HostSectionSlug) => void;
  onSelectHost: (serverId: string) => void;
  onAddHost: () => void;
}

export function SettingsSidebarContent({
  items,
  hostItems,
  selectedSectionId,
  selectedHostSection,
  activeHostServerId,
  sortedHosts,
  enableBuiltInDaemonOption,
  onSelectSection,
  onSelectHostSection,
  onSelectHost,
  onAddHost,
  iconSize,
  foreground,
  foregroundMuted,
}: SettingsSidebarContentProps) {
  const { t } = useTranslation();
  const hasHosts = sortedHosts.length > 0;
  return (
    <>
      <SidebarGroup label={t("settings.groups.app")}>
        {items.map((item) => (
          <SidebarSectionButton
            key={item.id}
            itemId={item.id}
            label={t(item.labelKey)}
            icon={item.icon}
            isSelected={selectedSectionId === item.id}
            onSelect={onSelectSection}
            iconSize={iconSize}
            foreground={foreground}
            foregroundMuted={foregroundMuted}
          />
        ))}
      </SidebarGroup>
      {hasHosts ? (
        <SidebarGroup label={t("settings.groups.host")}>
          <HostPicker
            activeServerId={activeHostServerId}
            sortedHosts={sortedHosts}
            onSelectHost={onSelectHost}
            onAddHost={onAddHost}
            enableBuiltInDaemonOption={enableBuiltInDaemonOption}
          />
          {hostItems.map((item) => (
            <SidebarHostSectionButton
              key={item.id}
              itemId={item.id}
              label={t(item.labelKey)}
              icon={item.icon}
              isSelected={selectedHostSection === item.id}
              onSelect={onSelectHostSection}
              iconSize={iconSize}
              foreground={foreground}
              foregroundMuted={foregroundMuted}
            />
          ))}
        </SidebarGroup>
      ) : (
        <SidebarGroup>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={t("settings.addHost")}
            onPress={onAddHost}
            testID="settings-add-host"
            style={sidebarItemStyle}
          >
            <Plus size={iconSize} color={foregroundMuted} strokeWidth={SIDEBAR_ICON_STROKE_WIDTH} />
            <Text style={sidebarStyles.label} numberOfLines={1}>
              {t("settings.addHost")}
            </Text>
          </Pressable>
          {enableBuiltInDaemonOption.visible ? (
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={t("settings.enableBuiltInDaemon")}
              onPress={enableBuiltInDaemonOption.onPress}
              testID="settings-enable-built-in-daemon"
              style={sidebarItemStyle}
            >
              <Server
                size={iconSize}
                color={foregroundMuted}
                strokeWidth={SIDEBAR_ICON_STROKE_WIDTH}
              />
              <Text style={sidebarStyles.label} numberOfLines={1}>
                {t("settings.enableBuiltInDaemon")}
              </Text>
            </Pressable>
          ) : null}
        </SidebarGroup>
      )}
    </>
  );
}

function sidebarItemStyle({
  hovered,
  pressed,
}: PressableStateCallbackType & { hovered?: boolean }) {
  return [
    sidebarStyles.item,
    Boolean(hovered) && sidebarStyles.itemHovered,
    pressed && sidebarStyles.itemPressed,
  ];
}

function selectedSidebarItemStyle({
  hovered,
  pressed,
}: PressableStateCallbackType & { hovered?: boolean }) {
  return [
    sidebarStyles.item,
    Boolean(hovered) && sidebarStyles.itemHovered,
    sidebarStyles.itemSelected,
    pressed && sidebarStyles.itemPressed,
  ];
}

interface SidebarSectionButtonProps extends SidebarAppearanceProps {
  itemId: SettingsSectionSlug;
  label: string;
  icon: ComponentType<{ size: number; color: string; strokeWidth?: number }>;
  isSelected: boolean;
  onSelect: (section: SettingsSectionSlug) => void;
}

// Lighter than lucide's default 2px stroke so the nav reads quieter than the content.
const SIDEBAR_ICON_STROKE_WIDTH = 1.5;

function SidebarSectionButton({
  itemId,
  label,
  icon: IconComponent,
  isSelected,
  onSelect,
  iconSize,
  foreground,
  foregroundMuted,
}: SidebarSectionButtonProps) {
  const handlePress = useCallback(() => {
    onSelect(itemId);
  }, [onSelect, itemId]);
  const accessibilityState = useMemo(() => ({ selected: isSelected }), [isSelected]);
  const labelStyle = useMemo(
    () => [sidebarStyles.label, isSelected && { color: foreground }],
    [isSelected, foreground],
  );
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={accessibilityState}
      onPress={handlePress}
      style={isSelected ? selectedSidebarItemStyle : sidebarItemStyle}
    >
      <IconComponent
        size={iconSize}
        color={isSelected ? foreground : foregroundMuted}
        strokeWidth={SIDEBAR_ICON_STROKE_WIDTH}
      />
      <Text style={labelStyle} numberOfLines={1}>
        {label}
      </Text>
    </Pressable>
  );
}

interface SidebarHostSectionButtonProps extends SidebarAppearanceProps {
  itemId: HostSectionSlug;
  label: string;
  icon: ComponentType<{ size: number; color: string; strokeWidth?: number }>;
  isSelected: boolean;
  onSelect: (section: HostSectionSlug) => void;
}

function SidebarHostSectionButton({
  itemId,
  label,
  icon: IconComponent,
  isSelected,
  onSelect,
  iconSize,
  foreground,
  foregroundMuted,
}: SidebarHostSectionButtonProps) {
  const handlePress = useCallback(() => {
    onSelect(itemId);
  }, [onSelect, itemId]);
  const accessibilityState = useMemo(() => ({ selected: isSelected }), [isSelected]);
  const labelStyle = useMemo(
    () => [sidebarStyles.label, isSelected && { color: foreground }],
    [isSelected, foreground],
  );
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={accessibilityState}
      onPress={handlePress}
      testID={`settings-host-section-${itemId}`}
      style={isSelected ? selectedSidebarItemStyle : sidebarItemStyle}
    >
      <IconComponent
        size={iconSize}
        color={isSelected ? foreground : foregroundMuted}
        strokeWidth={SIDEBAR_ICON_STROKE_WIDTH}
      />
      <Text style={labelStyle} numberOfLines={1}>
        {label}
      </Text>
    </Pressable>
  );
}

interface HostPickerProps {
  activeServerId: string | null;
  sortedHosts: HostProfile[];
  onSelectHost: (serverId: string) => void;
  onAddHost: () => void;
  enableBuiltInDaemonOption: EnableBuiltInDaemonOption;
}

/**
 * Scopes the host sections to a host. Reuses the canonical sidebar host
 * switcher pattern (left-sidebar.tsx): a quiet row-styled trigger opening a
 * <Combobox>. The local host is listed first, each row shows the connection it
 * is using right now; an "Add host" row is always reachable from the list —
 * even with a single host.
 */
function HostPicker({
  activeServerId,
  sortedHosts,
  onSelectHost,
  onAddHost,
  enableBuiltInDaemonOption,
}: HostPickerProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const triggerRef = useRef<View | null>(null);
  const activeHost =
    sortedHosts.find((host) => host.serverId === activeServerId) ?? sortedHosts[0] ?? null;

  const handleOpen = useCallback(() => setIsOpen(true), []);
  const hostOptionTestID = useCallback(
    (serverId: string) => `settings-host-picker-item-${serverId}`,
    [],
  );
  const triggerStyle = useCallback(
    ({ hovered = false, pressed }: PressableStateCallbackType & { hovered?: boolean }) => [
      sidebarStyles.pickerTrigger,
      hovered && sidebarStyles.pickerTriggerHovered,
      pressed && sidebarStyles.itemPressed,
    ],
    [],
  );

  return (
    <SharedHostPicker
      hosts={sortedHosts}
      value={activeServerId ?? ""}
      onSelect={onSelectHost}
      open={isOpen}
      onOpenChange={setIsOpen}
      anchorRef={triggerRef}
      includeAddHost
      onAddHost={onAddHost}
      includeEnableBuiltInDaemon={enableBuiltInDaemonOption.visible}
      onEnableBuiltInDaemon={enableBuiltInDaemonOption.onPress}
      showActiveConnection
      searchable={false}
      title={t("settings.hostPicker.switchHost")}
      desktopMinWidth={240}
      addHostTestID="settings-add-host"
      hostOptionTestID={hostOptionTestID}
    >
      <ComboboxTrigger
        ref={triggerRef}
        block
        style={triggerStyle}
        onPress={handleOpen}
        accessibilityRole="button"
        accessibilityLabel={t("settings.hostPicker.switchHost")}
        testID="settings-host-picker"
      >
        {activeHost ? (
          <View style={sidebarStyles.pickerTriggerDot}>
            <HostStatusDot serverId={activeHost.serverId} />
          </View>
        ) : null}
        <Text style={sidebarStyles.pickerTriggerLabel} numberOfLines={1}>
          {activeHost?.label ?? t("settings.groups.host")}
        </Text>
      </ComboboxTrigger>
    </SharedHostPicker>
  );
}

/** A labelled group of sidebar rows. On compact layouts the rows sit together in one island. */
interface SidebarGroupProps {
  label?: string;
  children: ReactNode;
}

function SidebarGroup({ label, children }: SidebarGroupProps) {
  return (
    <View style={sidebarStyles.group}>
      {label ? <Text style={sidebarStyles.groupLabel}>{label}</Text> : null}
      <View style={sidebarStyles.island}>{children}</View>
    </View>
  );
}

const sidebarStyles = StyleSheet.create((theme) => ({
  // Compact values draw each group as an island of tiles; desktop keeps a plain list.
  group: {
    paddingVertical: theme.spacing[2],
    paddingHorizontal: theme.spacing[2],
    gap: { xs: theme.spacing[1], md: theme.spacing[0.5] },
  },
  groupLabel: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.normal,
    color: theme.colors.foregroundExtraMuted,
    paddingHorizontal: { xs: theme.spacing[4], md: theme.spacing[2] },
    paddingVertical: theme.spacing[1],
  },
  island: {
    gap: { xs: 2, md: theme.spacing[0.5] },
    borderRadius: { xs: theme.borderRadius["2xl"], md: 0 },
    overflow: "hidden",
  },
  item: {
    flexDirection: "row",
    alignItems: "center",
    gap: { xs: theme.spacing[3], md: theme.spacing[2] },
    minHeight: { xs: 52, md: 28 },
    paddingVertical: theme.spacing[1],
    paddingHorizontal: { xs: theme.spacing[4], md: theme.spacing[2] },
    borderRadius: { xs: 0, md: theme.borderRadius.lg },
    backgroundColor: { xs: theme.colors.surface1, md: "transparent" },
  },
  itemHovered: {
    backgroundColor: theme.colors.surfaceSidebarHover,
  },
  itemSelected: {
    backgroundColor: theme.colors.surfaceSidebarHover,
  },
  // The workspace rows' pressed fill.
  itemPressed: {
    backgroundColor: theme.colors.surface2,
  },
  label: {
    fontSize: { xs: theme.fontSize.lg, md: theme.fontSize.base },
    color: theme.colors.foregroundMuted,
    fontWeight: theme.fontWeight.normal,
    flex: 1,
  },
  pickerTrigger: {
    flexDirection: "row",
    alignItems: "center",
    gap: { xs: theme.spacing[3], md: theme.spacing[2] },
    minHeight: { xs: 52, md: 28 },
    paddingVertical: theme.spacing[1],
    paddingHorizontal: { xs: theme.spacing[4], md: theme.spacing[2] },
    borderRadius: { xs: 0, md: theme.borderRadius.lg },
    backgroundColor: { xs: theme.colors.surface1, md: "transparent" },
  },
  pickerTriggerHovered: {
    backgroundColor: theme.colors.surfaceSidebarHover,
  },
  pickerTriggerLabel: {
    flex: 1,
    minWidth: 0,
    fontSize: { xs: theme.fontSize.lg, md: theme.fontSize.base },
    color: theme.colors.foreground,
    fontWeight: theme.fontWeight.normal,
  },
  // Match the setting items' icon footprint so the host label aligns with them.
  pickerTriggerDot: {
    width: densityIconBox("md"),
    height: densityIconBox("md"),
    alignItems: "center",
    justifyContent: "center",
  },
}));
