import { memo } from "react";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { useTranslation } from "react-i18next";
import { SkeletonPulse, useSkeletonPulse } from "@/components/ui/skeleton";
import { settingsStyles } from "@/styles/settings";

const SKELETON_ROW_KEYS = [0, 1, 2, 3].map((index) => `provider-skeleton-row-${index}`);

export const ProviderRowsSkeleton = memo(function ProviderRowsSkeleton() {
  const { t } = useTranslation();
  const pulse = useSkeletonPulse();
  return (
    <View
      style={settingsStyles.card}
      accessibilityLabel={t("settings.providers.loading")}
      testID="providers-skeleton"
    >
      {SKELETON_ROW_KEYS.map((key, index) => (
        <View
          key={key}
          style={[settingsStyles.row, index > 0 && settingsStyles.rowBorder, styles.row]}
        >
          <View style={styles.rowContent}>
            <SkeletonPulse pulse={pulse} style={styles.skeletonIcon} />
            <SkeletonPulse pulse={pulse} style={styles.skeletonTitle} />
          </View>
          <SkeletonPulse pulse={pulse} style={styles.skeletonSwitch} />
        </View>
      ))}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  row: {
    gap: theme.spacing[3],
    minHeight: 56,
  },
  rowContent: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  skeletonIcon: {
    width: 20,
    height: 20,
    marginLeft: theme.spacing[6],
    borderRadius: theme.borderRadius.md,
    backgroundColor: theme.colors.surface2,
  },
  skeletonTitle: {
    width: 160,
    height: 12,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: theme.colors.surface2,
  },
  skeletonSwitch: {
    width: 36,
    height: 20,
    borderRadius: theme.borderRadius.full,
    backgroundColor: theme.colors.surface2,
  },
}));
