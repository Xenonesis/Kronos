import React, { useMemo, useState, useEffect, useRef } from "react";
import { FlatList, Modal, Pressable, Text, TextInput, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import type { AstSymbol, AstSymbolKind } from "./types";

interface SymbolPaletteModalProps {
  visible: boolean;
  onClose: () => void;
  symbols: Array<AstSymbol & { displayName: string }>;
  onSelectSymbol: (symbol: AstSymbol) => void;
}

function getSymbolBadge(kind: AstSymbolKind): { label: string; color: string; bg: string } {
  switch (kind) {
    case "function":
    case "async_function":
    case "generator_function":
    case "arrow_function":
      return { label: "fn", color: "#a78bfa", bg: "rgba(167, 139, 250, 0.15)" };
    case "class":
      return { label: "class", color: "#f59e0b", bg: "rgba(245, 158, 11, 0.15)" };
    case "method":
    case "constructor":
      return { label: "meth", color: "#60a5fa", bg: "rgba(96, 165, 250, 0.15)" };
    case "interface":
      return { label: "intf", color: "#38bdf8", bg: "rgba(56, 189, 248, 0.15)" };
    case "type_alias":
      return { label: "type", color: "#2dd4bf", bg: "rgba(45, 212, 191, 0.15)" };
    case "enum":
    case "enum_member":
      return { label: "enum", color: "#fb923c", bg: "rgba(251, 146, 60, 0.15)" };
    case "constant":
      return { label: "const", color: "#4ade80", bg: "rgba(74, 222, 128, 0.15)" };
    case "variable":
      return { label: "var", color: "#94a3b8", bg: "rgba(148, 163, 184, 0.15)" };
    default:
      return { label: "sym", color: "#cbd5e1", bg: "rgba(203, 213, 225, 0.15)" };
  }
}

export function SymbolPaletteModal({
  visible,
  onClose,
  symbols,
  onSelectSymbol,
}: SymbolPaletteModalProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<TextInput>(null);

  useEffect(() => {
    if (visible) {
      setQuery("");
      setSelectedIndex(0);
      const timer = setTimeout(() => {
        inputRef.current?.focus();
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [visible]);

  const filteredSymbols = useMemo(() => {
    if (!query.trim()) return symbols;
    const lower = query.toLowerCase().trim();
    return symbols.filter((s) => s.displayName.toLowerCase().includes(lower));
  }, [symbols, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1 < filteredSymbols.length ? prev + 1 : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev - 1 >= 0 ? prev - 1 : filteredSymbols.length - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const target = filteredSymbols[selectedIndex];
      if (target) {
        onSelectSymbol(target);
        onClose();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  };

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <Pressable style={styles.backdrop} onPress={onClose}>
        <Pressable style={styles.dialog} onPress={(e) => e.stopPropagation()}>
          <View style={styles.header}>
            <TextInput
              ref={inputRef}
              style={styles.input}
              placeholder="Go to symbol in file... (@ for types, : for line)"
              placeholderTextColor="#64748b"
              value={query}
              onChangeText={setQuery}
              autoCapitalize="none"
              autoCorrect={false}
              spellCheck={false}
              // @ts-expect-error web keyboard support
              onKeyDown={handleKeyDown}
            />
          </View>
          <View style={styles.separator} />
          {filteredSymbols.length === 0 ? (
            <View style={styles.emptyState}>
              <Text style={styles.emptyText}>No matching symbols found</Text>
            </View>
          ) : (
            <FlatList
              data={filteredSymbols}
              keyExtractor={(item, index) => `${item.name}-${item.range.start_line}-${index}`}
              style={styles.list}
              keyboardShouldPersistTaps="always"
              renderItem={({ item, index }) => {
                const badge = getSymbolBadge(item.kind);
                const isSelected = index === selectedIndex;
                return (
                  <Pressable
                    style={[styles.itemRow, isSelected ? styles.itemRowSelected : null]}
                    onPress={() => {
                      onSelectSymbol(item);
                      onClose();
                    }}
                    onHoverIn={() => setSelectedIndex(index)}
                  >
                    <View style={[styles.badge, { backgroundColor: badge.bg }]}>
                      <Text style={[styles.badgeText, { color: badge.color }]}>{badge.label}</Text>
                    </View>
                    <Text style={styles.symbolName} numberOfLines={1}>
                      {item.displayName}
                    </Text>
                    <Text style={styles.lineIndicator}>Ln {item.range.start_line}</Text>
                  </Pressable>
                );
              }}
            />
          )}
        </Pressable>
      </Pressable>
    </Modal>
  );
}

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.5)",
    justifyContent: "flex-start",
    alignItems: "center",
    paddingTop: 80,
  },
  dialog: {
    width: "90%",
    maxWidth: 620,
    maxHeight: 460,
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 8 },
    shadowOpacity: 0.35,
    shadowRadius: 20,
    elevation: 10,
    overflow: "hidden",
  },
  header: {
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
  },
  input: {
    height: 40,
    fontSize: theme.fontSize.base,
    color: theme.colors.foreground,
    fontFamily: theme.fontFamily.ui,
    paddingHorizontal: theme.spacing[2],
  },
  separator: {
    height: 1,
    backgroundColor: theme.colors.border,
  },
  list: {
    maxHeight: 380,
  },
  itemRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    gap: theme.spacing[2],
  },
  itemRowSelected: {
    backgroundColor: theme.colors.surface1,
  },
  badge: {
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  badgeText: {
    fontSize: 11,
    fontFamily: theme.fontFamily.mono,
    fontWeight: "bold",
  },
  symbolName: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.foreground,
  },
  lineIndicator: {
    fontSize: 11,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.foregroundExtraMuted,
  },
  emptyState: {
    padding: theme.spacing[6],
    alignItems: "center",
    justifyContent: "center",
  },
  emptyText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
}));
