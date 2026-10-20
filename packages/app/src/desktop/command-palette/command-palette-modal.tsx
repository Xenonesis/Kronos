import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Modal, Platform, StyleSheet, Text, View, type GestureResponderEvent } from "react-native";
import { Search, Bot, Folder, Play, Globe } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { EditingTextInput, type EditingTextInputHandle } from "@/components/ui/text-input";
import type { PaletteItem } from "./types";

export interface CommandPaletteModalProps {
  isOpen: boolean;
  onClose: () => void;
  items: PaletteItem[];
  testID?: string;
}

/** How many matches the palette shows at once. */
const MAX_VISIBLE_ITEMS = 10;

function CategoryIcon({ category }: { category: PaletteItem["category"] }): React.ReactElement {
  switch (category) {
    case "agents":
      return <Bot size={16} color="#3b82f6" />;
    case "workspaces":
      return <Folder size={16} color="#eab308" />;
    case "navigation":
      return <Globe size={16} color="#10b981" />;
    case "actions":
    default:
      return <Play size={16} color="#a855f7" />;
  }
}

interface PaletteResultRowProps {
  item: PaletteItem;
  index: number;
  isSelected: boolean;
  onClose: () => void;
  onHover: (index: number) => void;
}

/**
 * One match. Its press and hover handlers live here, so neither is rebuilt in the parent's
 * render scope.
 */
function PaletteResultRow({
  item,
  index,
  isSelected,
  onClose,
  onHover,
}: PaletteResultRowProps): React.ReactElement {
  const handlePress = useCallback(() => {
    onClose();
    void item.run();
  }, [item, onClose]);

  const handleHoverIn = useCallback(() => {
    onHover(index);
  }, [index, onHover]);

  return (
    <PressHighlight
      accessibilityRole="button"
      style={[styles.itemRow, isSelected && styles.itemRowSelected]}
      onPress={handlePress}
      onHoverIn={handleHoverIn}
    >
      <View style={styles.itemIcon}>
        <CategoryIcon category={item.category} />
      </View>
      <View style={styles.itemTextContainer}>
        <Text style={[styles.itemTitle, isSelected && styles.itemTitleSelected]}>{item.title}</Text>
        {item.subtitle ? <Text style={styles.itemSubtitle}>{item.subtitle}</Text> : null}
      </View>
      {item.shortcut ? <Text style={styles.itemShortcut}>{item.shortcut}</Text> : null}
    </PressHighlight>
  );
}

export function CommandPaletteModal({
  isOpen,
  onClose,
  items,
  testID = "command-palette-modal",
}: CommandPaletteModalProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<EditingTextInputHandle | null>(null);

  // Filter items based on query
  const filteredItems = useMemo(() => {
    if (!query.trim()) return items;
    const lower = query.toLowerCase().trim();
    return items.filter(
      (item) =>
        item.title.toLowerCase().includes(lower) ||
        (item.subtitle && item.subtitle.toLowerCase().includes(lower)) ||
        (item.keywords && item.keywords.some((k) => k.toLowerCase().includes(lower))),
    );
  }, [items, query]);

  const visibleItems = useMemo(() => filteredItems.slice(0, MAX_VISIBLE_ITEMS), [filteredItems]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  // Focus input on open
  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => {
        inputRef.current?.focus();
      }, 50);
    }
  }, [isOpen]);

  const handleHover = useCallback((index: number) => {
    setSelectedIndex(index);
  }, []);

  const handleStopPropagation = useCallback((event: GestureResponderEvent) => {
    event.stopPropagation();
  }, []);

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1 < filteredItems.length ? prev + 1 : 0));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 >= 0 ? prev - 1 : filteredItems.length - 1));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const selected = filteredItems[selectedIndex];
        if (selected) {
          onClose();
          void selected.run();
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    },
    [filteredItems, onClose, selectedIndex],
  );

  if (!isOpen) return null;

  return (
    <Modal
      visible={isOpen}
      transparent
      animationType="fade"
      onRequestClose={onClose}
      testID={testID}
    >
      <PressHighlight
        accessibilityRole="button"
        accessibilityLabel="Close command palette"
        style={styles.backdrop}
        onPress={onClose}
      >
        <PressHighlight
          style={styles.container}
          onPress={handleStopPropagation}
          // @ts-expect-error onKeyDown for web
          onKeyDown={Platform.OS === "web" ? handleKeyDown : undefined}
        >
          {/* Search Input Bar */}
          <View style={styles.searchBar}>
            <Search size={18} color="#94a3b8" />
            <EditingTextInput
              ref={inputRef}
              style={styles.input}
              placeholder="Type a command, search agents, or open workspace..."
              placeholderTextColor="#64748b"
              onChangeText={setQuery}
              autoFocus
            />
            <Text style={styles.kbdBadge}>ESC to close</Text>
          </View>

          {/* Results List */}
          <View style={styles.list}>
            {visibleItems.length === 0 ? (
              <View style={styles.emptyContainer}>
                <Text style={styles.emptyText}>No matching commands or workspaces</Text>
              </View>
            ) : (
              visibleItems.map((item, index) => (
                <PaletteResultRow
                  key={item.id}
                  item={item}
                  index={index}
                  isSelected={index === selectedIndex}
                  onClose={onClose}
                  onHover={handleHover}
                />
              ))
            )}
          </View>
        </PressHighlight>
      </PressHighlight>
    </Modal>
  );
}

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.6)",
    justifyContent: "flex-start",
    alignItems: "center",
    paddingTop: 100,
  },
  container: {
    width: "90%",
    maxWidth: 600,
    backgroundColor: "#1e293b",
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#334155",
    overflow: "hidden",
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 10 },
    shadowOpacity: 0.5,
    shadowRadius: 20,
    elevation: 20,
  },
  searchBar: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 16,
    height: 52,
    borderBottomWidth: 1,
    borderBottomColor: "#334155",
    gap: 12,
  },
  input: {
    flex: 1,
    color: "#f8fafc",
    fontSize: 15,
    paddingVertical: 8,
  },
  kbdBadge: {
    color: "#64748b",
    fontSize: 11,
    backgroundColor: "#0f172a",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
    borderWidth: 1,
    borderColor: "#334155",
  },
  list: {
    paddingVertical: 8,
    maxHeight: 380,
  },
  itemRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 16,
    paddingVertical: 10,
    gap: 12,
  },
  itemRowSelected: {
    backgroundColor: "#334155",
  },
  itemIcon: {
    width: 28,
    height: 28,
    borderRadius: 6,
    backgroundColor: "#0f172a",
    justifyContent: "center",
    alignItems: "center",
  },
  itemTextContainer: {
    flex: 1,
  },
  itemTitle: {
    color: "#e2e8f0",
    fontSize: 14,
    fontWeight: "500",
  },
  itemTitleSelected: {
    color: "#ffffff",
    fontWeight: "600",
  },
  itemSubtitle: {
    color: "#94a3b8",
    fontSize: 12,
  },
  itemShortcut: {
    color: "#64748b",
    fontSize: 11,
    fontFamily: "monospace",
  },
  emptyContainer: {
    padding: 24,
    alignItems: "center",
  },
  emptyText: {
    color: "#64748b",
    fontSize: 13,
  },
});
