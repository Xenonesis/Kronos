import { memo, useCallback, useEffect, useMemo, useState, useRef } from "react";
import type { NativeSyntheticEvent, TextInputKeyPressEventData } from "react-native";
import { View, Text, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import {
  GitCompare,
  GitBranch,
  Terminal,
  FileCode,
  Folder,
  Search,
  FileText,
  X,
  File,
  StickyNote,
} from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import {
  EditingTextInput as TextInput,
  type EditingTextInputHandle,
} from "@/components/ui/text-input";
import { STATIC_MENTIONS, type MentionItem } from "./types";
import type { DaemonClient } from "@getpaseo/client/internal/daemon-client";

export type MentionFilterTab = "all" | "files" | "folders" | "context";

export interface MentionPaletteProps {
  visible: boolean;
  onSelect: (item: MentionItem) => void;
  onClose?: () => void;
  query?: string;
  onQueryChange?: (nextQuery: string) => void;
  cwd?: string;
  client?: DaemonClient | null;
}

const ThemedGitCompare = withUnistyles(GitCompare);
const ThemedGitBranch = withUnistyles(GitBranch);
const ThemedTerminal = withUnistyles(Terminal);
const ThemedFileCode = withUnistyles(FileCode);
const ThemedFolder = withUnistyles(Folder);
const ThemedSearch = withUnistyles(Search);
const ThemedFileText = withUnistyles(FileText);
const ThemedX = withUnistyles(X);
const ThemedFile = withUnistyles(File);
const ThemedStickyNote = withUnistyles(StickyNote);

interface MentionRowProps {
  item: MentionItem;
  isSelected?: boolean;
  onSelect: (item: MentionItem) => void;
}

const MentionRow = memo(function MentionRow({ item, isSelected, onSelect }: MentionRowProps) {
  const handlePress = useCallback(() => {
    onSelect(item);
  }, [item, onSelect]);

  const renderIcon = () => {
    const size = 14;
    switch (item.kind) {
      case "folder":
        return <ThemedFolder size={size} />;
      case "file": {
        const ext = item.path?.split(".").pop()?.toLowerCase();
        if (
          ext &&
          [
            "ts",
            "tsx",
            "js",
            "jsx",
            "rs",
            "py",
            "go",
            "c",
            "cpp",
            "java",
            "php",
            "html",
            "css",
            "scss",
            "vue",
            "svelte",
          ].includes(ext)
        ) {
          return <ThemedFileCode size={size} />;
        }
        if (ext && ["json", "yaml", "yml", "toml", "md", "txt", "env"].includes(ext)) {
          return <ThemedFileText size={size} />;
        }
        return <ThemedFileCode size={size} />;
      }
      case "git_diff":
        return <ThemedGitCompare size={size} />;
      case "git_status":
        return <ThemedGitBranch size={size} />;
      case "terminal":
        return <ThemedTerminal size={size} />;
      case "note":
        return <ThemedStickyNote size={size} />;
      default:
        return <ThemedFile size={size} />;
    }
  };

  return (
    <PressHighlight
      style={[styles.item, isSelected ? styles.itemSelected : null]}
      onPress={handlePress}
      accessibilityRole="button"
      accessibilityLabel={item.label}
    >
      <View style={styles.iconContainer}>{renderIcon()}</View>
      <View style={styles.textContainer}>
        <Text style={styles.label} numberOfLines={1}>
          {item.label}
        </Text>
        {item.detail ? (
          <Text style={styles.detail} numberOfLines={1}>
            {item.detail}
          </Text>
        ) : null}
      </View>
      <Text style={styles.token}>{item.insertToken.trim()}</Text>
    </PressHighlight>
  );
});

function parseSuggestionsToItems(payload: {
  entries?: Array<{ path: string; kind: string }>;
  directories?: string[];
}): MentionItem[] {
  const items: MentionItem[] = [];
  const seen = new Set<string>();

  if (payload.entries && payload.entries.length > 0) {
    for (const entry of payload.entries) {
      if (seen.has(entry.path)) continue;
      seen.add(entry.path);

      const isDir = entry.kind === "directory";
      const normalized = entry.path.replace(/\\/g, "/");
      const parts = normalized.split("/");
      const base = parts[parts.length - 1] || parts[parts.length - 2] || normalized;
      const parent = parts.length > 1 ? parts.slice(0, -1).join("/") : undefined;

      items.push({
        id: `${isDir ? "folder" : "file"}:${normalized}`,
        kind: isDir ? "folder" : "file",
        label: isDir ? `${base}/` : base,
        detail: parent,
        insertToken: isDir ? `@folder:${normalized}/ ` : `@file:${normalized} `,
        path: normalized,
      });
    }
    return items;
  }

  if (payload.directories && payload.directories.length > 0) {
    for (const dir of payload.directories) {
      if (seen.has(dir)) continue;
      seen.add(dir);
      const normalized = dir.replace(/\\/g, "/");
      const parts = normalized.split("/");
      const base = parts[parts.length - 1] || normalized;
      items.push({
        id: `folder:${normalized}`,
        kind: "folder",
        label: `${base}/`,
        detail: normalized,
        insertToken: `@folder:${normalized}/ `,
        path: normalized,
      });
    }
  }

  return items;
}

export const MentionPalette = memo(function MentionPalette({
  visible,
  onSelect,
  onClose,
  query = "",
  onQueryChange,
  cwd,
  client,
}: MentionPaletteProps) {
  const [activeTab, setActiveTab] = useState<MentionFilterTab>("all");
  const [internalQuery, setInternalQuery] = useState(query);
  const [workspaceItems, setWorkspaceItems] = useState<MentionItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const searchInputRef = useRef<EditingTextInputHandle | null>(null);

  useEffect(() => {
    setInternalQuery(query);
    if (searchInputRef.current && searchInputRef.current.getText() !== query) {
      searchInputRef.current.replaceText(query);
    }
  }, [query]);

  const handleQueryChange = useCallback(
    (next: string) => {
      setInternalQuery(next);
      onQueryChange?.(next);
      setSelectedIndex(0);
    },
    [onQueryChange],
  );

  const handleClearQuery = useCallback(() => {
    searchInputRef.current?.reset();
    handleQueryChange("");
  }, [handleQueryChange]);

  const handleSelectTabAll = useCallback(() => {
    setActiveTab("all");
    setSelectedIndex(0);
  }, []);

  const handleSelectTabFiles = useCallback(() => {
    setActiveTab("files");
    setSelectedIndex(0);
  }, []);

  const handleSelectTabFolders = useCallback(() => {
    setActiveTab("folders");
    setSelectedIndex(0);
  }, []);

  const handleSelectTabContext = useCallback(() => {
    setActiveTab("context");
    setSelectedIndex(0);
  }, []);

  useEffect(() => {
    if (!visible) return;
    if (!client || !cwd) {
      setWorkspaceItems([]);
      return;
    }

    let isCancelled = false;
    setIsLoading(true);

    const timer = setTimeout(async () => {
      try {
        const payload = await client.getDirectorySuggestions({
          cwd,
          query: internalQuery.trim(),
          limit: 35,
          includeFiles: true,
          includeDirectories: true,
        });

        if (isCancelled) return;
        setWorkspaceItems(parseSuggestionsToItems(payload));
      } catch {
        if (!isCancelled) setWorkspaceItems([]);
      } finally {
        if (!isCancelled) setIsLoading(false);
      }
    }, 120);

    return () => {
      isCancelled = true;
      clearTimeout(timer);
    };
  }, [visible, client, cwd, internalQuery]);

  const displayedItems = useMemo(() => {
    const q = internalQuery.toLowerCase().trim();
    const matchedStatic = STATIC_MENTIONS.filter(
      (m) => !q || m.label.toLowerCase().includes(q) || m.insertToken.toLowerCase().includes(q),
    );

    if (activeTab === "context") return matchedStatic;
    if (activeTab === "files") return workspaceItems.filter((i) => i.kind === "file");
    if (activeTab === "folders") return workspaceItems.filter((i) => i.kind === "folder");

    return [...workspaceItems, ...matchedStatic];
  }, [activeTab, internalQuery, workspaceItems]);

  const handleKeyDown = useCallback(
    (e: NativeSyntheticEvent<TextInputKeyPressEventData>) => {
      const key = e.nativeEvent?.key;
      if (key === "ArrowDown") {
        e.preventDefault?.();
        setSelectedIndex((prev) => (prev + 1) % Math.max(1, displayedItems.length));
      } else if (key === "ArrowUp") {
        e.preventDefault?.();
        setSelectedIndex((prev) => (prev <= 0 ? Math.max(0, displayedItems.length - 1) : prev - 1));
      } else if (key === "Enter" || key === "Tab") {
        if (displayedItems[selectedIndex]) {
          e.preventDefault?.();
          onSelect(displayedItems[selectedIndex]);
        }
      } else if (key === "Escape") {
        e.preventDefault?.();
        onClose?.();
      }
    },
    [displayedItems, selectedIndex, onSelect, onClose],
  );

  if (!visible) return null;

  return (
    <View style={styles.container} testID="mention-palette">
      <View style={styles.header}>
        <View style={styles.headerTitleRow}>
          <Text style={styles.headerText}>Attach Context (@)</Text>
          {onClose ? (
            <PressHighlight
              onPress={onClose}
              accessibilityRole="button"
              accessibilityLabel="Close mention palette"
              style={styles.closeBtn}
            >
              <ThemedX size={13} />
            </PressHighlight>
          ) : null}
        </View>

        <View style={styles.tabBar}>
          <PressHighlight
            style={[styles.tab, activeTab === "all" ? styles.tabActive : null]}
            onPress={handleSelectTabAll}
            accessibilityRole="button"
            accessibilityLabel="All items"
          >
            <Text style={[styles.tabText, activeTab === "all" ? styles.tabTextActive : null]}>
              All
            </Text>
          </PressHighlight>

          <PressHighlight
            style={[styles.tab, activeTab === "files" ? styles.tabActive : null]}
            onPress={handleSelectTabFiles}
            accessibilityRole="button"
            accessibilityLabel="Files only"
          >
            <Text style={[styles.tabText, activeTab === "files" ? styles.tabTextActive : null]}>
              Files
            </Text>
          </PressHighlight>

          <PressHighlight
            style={[styles.tab, activeTab === "folders" ? styles.tabActive : null]}
            onPress={handleSelectTabFolders}
            accessibilityRole="button"
            accessibilityLabel="Folders only"
          >
            <Text style={[styles.tabText, activeTab === "folders" ? styles.tabTextActive : null]}>
              Folders
            </Text>
          </PressHighlight>

          <PressHighlight
            style={[styles.tab, activeTab === "context" ? styles.tabActive : null]}
            onPress={handleSelectTabContext}
            accessibilityRole="button"
            accessibilityLabel="Git and system context"
          >
            <Text style={[styles.tabText, activeTab === "context" ? styles.tabTextActive : null]}>
              Git & System
            </Text>
          </PressHighlight>
        </View>

        <View style={styles.searchRow}>
          <ThemedSearch size={13} />
          <TextInput
            ref={searchInputRef}
            style={styles.searchInput}
            placeholder="Search files, folders, or context..."
            placeholderTextColor="#888"
            initialValue={internalQuery}
            onChangeText={handleQueryChange}
            onKeyPress={handleKeyDown}
            autoCapitalize="none"
            autoCorrect={false}
          />
          {internalQuery ? (
            <PressHighlight
              onPress={handleClearQuery}
              accessibilityRole="button"
              accessibilityLabel="Clear search query"
              style={styles.clearSearchBtn}
            >
              <ThemedX size={11} />
            </PressHighlight>
          ) : null}
        </View>
      </View>

      <ScrollView
        style={styles.scrollList}
        keyboardShouldPersistTaps="handled"
        showsVerticalScrollIndicator={true}
      >
        {displayedItems.length > 0 ? (
          displayedItems.map((item, index) => (
            <MentionRow
              key={item.id}
              item={item}
              isSelected={index === selectedIndex}
              onSelect={onSelect}
            />
          ))
        ) : (
          <View style={styles.emptyContainer}>
            <Text style={styles.emptyText}>
              {isLoading
                ? "Searching workspace files & folders..."
                : "No matching files, folders, or context found."}
            </Text>
          </View>
        )}
      </ScrollView>
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
    maxHeight: 340,
  },
  header: {
    paddingHorizontal: 12,
    paddingTop: 8,
    paddingBottom: 6,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface2,
    gap: 6,
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  headerText: {
    fontSize: 11,
    fontWeight: "700",
    color: theme.colors.foregroundMuted,
    textTransform: "uppercase",
    letterSpacing: 0.6,
  },
  closeBtn: {
    padding: 2,
    borderRadius: theme.borderRadius.sm,
  },
  tabBar: {
    flexDirection: "row",
    gap: 4,
  },
  tab: {
    paddingHorizontal: 8,
    paddingVertical: 3,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: "transparent",
  },
  tabActive: {
    backgroundColor: theme.colors.surface0,
  },
  tabText: {
    fontSize: 11,
    fontWeight: "500",
    color: theme.colors.foregroundMuted,
  },
  tabTextActive: {
    color: theme.colors.foreground,
    fontWeight: "600",
  },
  searchRow: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.sm,
    paddingHorizontal: 8,
    paddingVertical: 4,
    gap: 6,
  },
  searchInput: {
    flex: 1,
    fontSize: 12,
    color: theme.colors.foreground,
    padding: 0,
  },
  clearSearchBtn: {
    padding: 2,
    borderRadius: theme.borderRadius.sm,
  },
  scrollList: {
    maxHeight: 220,
    paddingVertical: 4,
  },
  item: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 12,
    paddingVertical: 7,
    gap: 8,
  },
  itemSelected: {
    backgroundColor: theme.colors.surface2,
  },
  iconContainer: {
    width: 20,
    alignItems: "center",
    justifyContent: "center",
  },
  textContainer: {
    flex: 1,
    minWidth: 0,
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
  emptyContainer: {
    paddingVertical: 18,
    paddingHorizontal: 16,
    alignItems: "center",
  },
  emptyText: {
    fontSize: 12,
    color: theme.colors.foregroundMuted,
  },
}));
