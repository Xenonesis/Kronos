import { memo, useState, useCallback, useMemo } from "react";
import { View, Text, Modal, Pressable, TextInput, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Store, X, Search, Check, Download, ExternalLink, ShieldCheck } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { CURATED_MCP_CATALOG, type McpPresetItem } from "@/desktop/data/mcp-catalog-presets";
import type { McpServerConfig } from "@/desktop/components/mcp-servers-card";

export interface McpRegistryCatalogProps {
  visible: boolean;
  onClose: () => void;
  installedServerIds: string[];
  onInstall: (server: McpServerConfig) => void;
}

const CATEGORIES = [
  "All",
  "Developer Tools",
  "Filesystem & DB",
  "Search & Web",
  "Communication",
] as const;

export const McpRegistryCatalog = memo(function McpRegistryCatalog({
  visible,
  onClose,
  installedServerIds,
  onInstall,
}: McpRegistryCatalogProps) {
  const { theme } = useUnistyles();
  const [selectedCategory, setSelectedCategory] = useState<string>("All");
  const [searchQuery, setSearchQuery] = useState("");

  const filteredPresets = useMemo(() => {
    return CURATED_MCP_CATALOG.filter((item) => {
      const matchesCategory = selectedCategory === "All" || item.category === selectedCategory;
      const q = searchQuery.toLowerCase().trim();
      const matchesSearch =
        !q ||
        item.name.toLowerCase().includes(q) ||
        item.description.toLowerCase().includes(q) ||
        item.config.command.toLowerCase().includes(q);
      return matchesCategory && matchesSearch;
    });
  }, [selectedCategory, searchQuery]);

  const handleInstall = useCallback(
    (item: McpPresetItem) => {
      onInstall(item.config);
    },
    [onInstall],
  );

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.card}>
          {/* Header */}
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <Store size={18} color={theme.colors.accent} />
              <View>
                <Text style={styles.titleText}>Community MCP Registry</Text>
                <Text style={styles.subtitleText}>
                  Browse and 1-click install verified Model Context Protocol servers
                </Text>
              </View>
            </View>
            <Pressable onPress={onClose} style={styles.closeBtn}>
              <X size={18} color={theme.colors.foregroundMuted} />
            </Pressable>
          </View>

          {/* Filter & Search Bar */}
          <View style={styles.filterBar}>
            <View style={styles.searchBox}>
              <Search size={14} color={theme.colors.foregroundMuted} />
              <TextInput
                value={searchQuery}
                onChangeText={setSearchQuery}
                placeholder="Search MCP servers..."
                placeholderTextColor={theme.colors.foregroundMuted}
                style={styles.searchInput}
              />
            </View>

            <ScrollView
              horizontal
              showsHorizontalScrollIndicator={false}
              style={styles.categoryScroll}
            >
              <View style={styles.categoryRow}>
                {CATEGORIES.map((cat) => {
                  const isSelected = selectedCategory === cat;
                  return (
                    <Pressable
                      key={cat}
                      onPress={() => setSelectedCategory(cat)}
                      style={[styles.categoryPill, isSelected && styles.categoryPillSelected]}
                    >
                      <Text
                        style={[styles.categoryText, isSelected && styles.categoryTextSelected]}
                      >
                        {cat}
                      </Text>
                    </Pressable>
                  );
                })}
              </View>
            </ScrollView>
          </View>

          {/* Preset Cards List */}
          <ScrollView style={styles.catalogList}>
            <View style={styles.catalogGrid}>
              {filteredPresets.map((item) => {
                const isInstalled = installedServerIds.includes(item.config.id);
                return (
                  <View key={item.id} style={styles.itemCard}>
                    <View style={styles.cardTop}>
                      <View style={styles.nameRow}>
                        <Text style={styles.itemName}>{item.name}</Text>
                        <View style={styles.categoryBadge}>
                          <Text style={styles.categoryBadgeText}>{item.category}</Text>
                        </View>
                      </View>
                      <Text style={styles.itemAuthor}>by {item.author}</Text>
                    </View>

                    <Text style={styles.itemDesc}>{item.description}</Text>

                    <View style={styles.commandPreview}>
                      <Text style={styles.commandText} numberOfLines={1}>
                        {item.config.command} {item.config.args.join(" ")}
                      </Text>
                    </View>

                    {item.envRequirements && item.envRequirements.length > 0 && (
                      <View style={styles.envNotice}>
                        <ShieldCheck size={12} color={theme.colors.accent} />
                        <Text style={styles.envNoticeText}>
                          Requires: {item.envRequirements.join(", ")}
                        </Text>
                      </View>
                    )}

                    <View style={styles.cardBottom}>
                      {isInstalled ? (
                        <View style={styles.installedBadge}>
                          <Check size={13} color={theme.colors.success ?? "#16a34a"} />
                          <Text style={styles.installedText}>Installed</Text>
                        </View>
                      ) : (
                        <Button size="sm" variant="default" onPress={() => handleInstall(item)}>
                          <Download size={13} color="#ffffff" />
                          <Text style={styles.installBtnText}>Install to Kronos</Text>
                        </Button>
                      )}
                    </View>
                  </View>
                );
              })}
            </View>
          </ScrollView>

          {/* Footer */}
          <View style={styles.footer}>
            <Text style={styles.footerHint}>
              Servers run locally via stdio / JSON-RPC in pure native isolation.
            </Text>
            <Button size="sm" variant="outline" onPress={onClose}>
              Done
            </Button>
          </View>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.65)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  card: {
    width: "92%",
    maxWidth: 820,
    maxHeight: 640,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    flex: 1,
  },
  titleText: {
    fontSize: theme.fontSize.base,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  subtitleText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  closeBtn: {
    padding: theme.spacing[1],
  },
  filterBar: {
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[2],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
    gap: theme.spacing[2],
  },
  searchBox: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[1],
    borderRadius: theme.borderRadius.sm,
  },
  searchInput: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    padding: 0,
  },
  categoryScroll: {
    maxHeight: 32,
  },
  categoryRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  categoryPill: {
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: theme.colors.surface2,
  },
  categoryPillSelected: {
    backgroundColor: theme.colors.accent,
  },
  categoryText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
    fontWeight: "500",
  },
  categoryTextSelected: {
    color: "#ffffff",
    fontWeight: "600",
  },
  catalogList: {
    maxHeight: 400,
    padding: theme.spacing[4],
  },
  catalogGrid: {
    gap: theme.spacing[3],
  },
  itemCard: {
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[3],
    gap: theme.spacing[2],
  },
  cardTop: {
    gap: 2,
  },
  nameRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  itemName: {
    fontSize: theme.fontSize.sm,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  categoryBadge: {
    backgroundColor: "rgba(59, 130, 246, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  categoryBadgeText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.accent,
    fontWeight: "500",
  },
  itemAuthor: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  itemDesc: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    lineHeight: 18,
  },
  commandPreview: {
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
  },
  commandText: {
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  envNotice: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  envNoticeText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.accent,
  },
  cardBottom: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "flex-end",
    paddingTop: theme.spacing[1],
  },
  installedBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: "rgba(22, 163, 74, 0.12)",
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
  },
  installedText: {
    fontSize: theme.fontSize.code,
    fontWeight: "600",
    color: theme.colors.success ?? "#16a34a",
  },
  installBtnText: {
    color: "#ffffff",
    fontWeight: "600",
    fontSize: theme.fontSize.code,
  },
  footer: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  footerHint: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
}));
