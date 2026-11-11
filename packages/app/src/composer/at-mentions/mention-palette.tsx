import { memo, useCallback } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { GitCompare, GitBranch, Terminal, FileCode } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { STATIC_MENTIONS, type MentionItem } from "./types";

interface MentionPaletteProps {
  visible: boolean;
  onSelect: (item: MentionItem) => void;
  onClose?: () => void;
}

const ThemedGitCompare = withUnistyles(GitCompare);
const ThemedGitBranch = withUnistyles(GitBranch);
const ThemedTerminal = withUnistyles(Terminal);
const ThemedFileCode = withUnistyles(FileCode);

interface MentionRowProps {
  item: MentionItem;
  onSelect: (item: MentionItem) => void;
}

const MentionRow = memo(function MentionRow({ item, onSelect }: MentionRowProps) {
  const handlePress = useCallback(() => {
    onSelect(item);
  }, [item, onSelect]);

  const renderIcon = () => {
    const size = 14;
    switch (item.kind) {
      case "git_diff":
        return <ThemedGitCompare size={size} />;
      case "git_status":
        return <ThemedGitBranch size={size} />;
      case "terminal":
        return <ThemedTerminal size={size} />;
      default:
        return <ThemedFileCode size={size} />;
    }
  };

  return (
    <PressHighlight
      style={styles.item}
      onPress={handlePress}
      accessibilityRole="button"
      accessibilityLabel={item.label}
    >
      <View style={styles.iconContainer}>{renderIcon()}</View>
      <View style={styles.textContainer}>
        <Text style={styles.label}>{item.label}</Text>
        {item.detail ? <Text style={styles.detail}>{item.detail}</Text> : null}
      </View>
      <Text style={styles.token}>{item.insertToken.trim()}</Text>
    </PressHighlight>
  );
});

export const MentionPalette = memo(function MentionPalette({
  visible,
  onSelect,
}: MentionPaletteProps) {
  if (!visible) return null;

  return (
    <View style={styles.container} testID="mention-palette">
      <View style={styles.header}>
        <Text style={styles.headerText}>Attach Context (@)</Text>
      </View>
      <View style={styles.list}>
        {STATIC_MENTIONS.map((item) => (
          <MentionRow key={item.id} item={item} onSelect={onSelect} />
        ))}
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    backgroundColor: theme.colors.surface1,
    borderColor: theme.colors.border,
    borderWidth: 1,
    borderRadius: theme.borderRadius.md,
    overflow: "hidden",
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.15,
    shadowRadius: 8,
    elevation: 6,
    marginBottom: 6,
    marginHorizontal: 8,
  },
  header: {
    paddingHorizontal: 12,
    paddingVertical: 6,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface2,
  },
  headerText: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  list: {
    paddingVertical: 4,
  },
  item: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 12,
    paddingVertical: 7,
    gap: 8,
  },
  iconContainer: {
    width: 20,
    alignItems: "center",
    justifyContent: "center",
  },
  textContainer: {
    flex: 1,
  },
  label: {
    fontSize: 13,
    fontWeight: "500",
    color: theme.colors.foreground,
  },
  detail: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    marginTop: 1,
  },
  token: {
    fontSize: 11,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.primary,
    backgroundColor: theme.colors.surface0,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: theme.borderRadius.sm,
  },
}));
