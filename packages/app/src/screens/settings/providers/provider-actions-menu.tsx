import { memo, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { GestureResponderEvent, PressableStateCallbackType } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { MoreHorizontal, Trash2 } from "lucide-react-native";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

interface ProviderActionsMenuProps {
  providerId: string;
  providerLabel: string;
  isRemoving: boolean;
  onRemove: (providerId: string, providerLabel: string) => void;
}

function stopPressInPropagation(event: GestureResponderEvent) {
  event.stopPropagation();
}

export const ProviderActionsMenu = memo(function ProviderActionsMenu({
  providerId,
  providerLabel,
  isRemoving,
  onRemove,
}: ProviderActionsMenuProps) {
  const { t } = useTranslation();
  const handleRemove = useCallback(() => {
    onRemove(providerId, providerLabel);
  }, [onRemove, providerId, providerLabel]);

  const triggerStyle = useCallback(
    ({
      pressed,
      hovered,
      open,
    }: PressableStateCallbackType & { hovered?: boolean; open?: boolean }) => [
      styles.menuButton,
      (hovered || open) && styles.menuButtonHovered,
      pressed && styles.menuButtonPressed,
    ],
    [],
  );
  const trashLeading = useMemo(() => <Trash2 size={16} color={styles.dangerColor.color} />, []);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        disabled={isRemoving}
        hitSlop={8}
        onPressIn={stopPressInPropagation}
        style={triggerStyle}
        accessibilityRole="button"
        accessibilityLabel={t("settings.providers.actions.menu", { name: providerLabel })}
        testID={`provider-actions-${providerId}`}
      >
        {({ hovered, open }) => (
          <MoreHorizontal
            size={styles.menuIcon.fontSize}
            color={hovered || open ? styles.foreground.color : styles.foregroundMuted.color}
          />
        )}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" width={220}>
        <DropdownMenuItem
          destructive
          leading={trashLeading}
          onSelect={handleRemove}
          status={isRemoving ? "pending" : "idle"}
          pendingLabel={t("settings.providers.actions.removing")}
          testID={`provider-remove-${providerId}`}
        >
          {t("settings.providers.actions.remove")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
});

const styles = StyleSheet.create((theme) => ({
  menuButton: {
    width: 32,
    height: 32,
    borderRadius: theme.borderRadius.lg,
    alignItems: "center",
    justifyContent: "center",
  },
  menuButtonHovered: {
    backgroundColor: theme.colors.surface2,
  },
  menuButtonPressed: {
    backgroundColor: theme.colors.surface3,
  },
  menuIcon: {
    fontSize: theme.iconSize.sm,
  },
  foreground: {
    color: theme.colors.foreground,
  },
  foregroundMuted: {
    color: theme.colors.foregroundMuted,
  },
  dangerColor: {
    color: theme.colors.statusDanger,
  },
}));
