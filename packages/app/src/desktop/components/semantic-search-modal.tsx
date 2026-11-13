import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Sparkles, Search, X, FileCode, Plus } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { PressHighlight } from "@/components/ui/press-highlight";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { invokeTauri } from "@/utils/tauri-invoke";
import { useToast } from "@/contexts/toast-context";
import { useSessionStore } from "@/stores/session-store";
import { useDraftStore } from "@/stores/draft-store";

const ThemedSearchIcon = withUnistyles(Sparkles, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedFileIcon = withUnistyles(FileCode, (theme) => ({ color: theme.colors.accent }));

export interface SemanticCodeChunk {
  filePath: string;
  relativePath: string;
  lineStart: number;
  lineEnd: number;
  content: string;
  score: number;
}

const ChunkCard = memo(function ChunkCard({
  chunk,
  onInsert,
}: {
  chunk: SemanticCodeChunk;
  onInsert: (chunk: SemanticCodeChunk) => void;
}) {
  const handleInsert = useCallback(() => {
    onInsert(chunk);
  }, [chunk, onInsert]);

  return (
    <View style={styles.chunkCard}>
      <View style={styles.chunkHeader}>
        <View style={styles.chunkTitleRow}>
          <ThemedFileIcon size={13} />
          <Text style={styles.chunkPath} numberOfLines={1}>
            {chunk.relativePath}:{chunk.lineStart}-{chunk.lineEnd}
          </Text>
        </View>
        <View style={styles.chunkActions}>
          <Text style={styles.scoreBadge}>{chunk.score.toFixed(2)}</Text>
          <PressHighlight onPress={handleInsert} style={styles.insertBtn}>
            <Plus size={12} color="#ffffff" />
            <Text style={styles.insertText}>Add to Context</Text>
          </PressHighlight>
        </View>
      </View>
      <Text style={styles.chunkCode} numberOfLines={10} selectable>
        {chunk.content}
      </Text>
    </View>
  );
});

export interface SemanticSearchModalProps {
  visible: boolean;
  cwd: string;
  serverId?: string;
  agentId?: string;
  onClose: () => void;
}

export const SemanticSearchModal = memo(function SemanticSearchModal({
  visible,
  cwd,
  serverId: propServerId,
  agentId: propAgentId,
  onClose,
}: SemanticSearchModalProps) {
  const [query, setQuery] = useState("");
  const [chunks, setChunks] = useState<SemanticCodeChunk[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [hasSearched, setHasSearched] = useState(false);
  const toast = useToast();

  useEffect(() => {
    if (visible) {
      setChunks([]);
      setHasSearched(false);
    }
  }, [visible]);

  const handleSearch = useCallback(async () => {
    const trimmed = query.trim();
    if (!trimmed) return;
    setIsSearching(true);
    try {
      const results = await invokeTauri<SemanticCodeChunk[]>("semantic_search_codebase", {
        req: { cwd, query: trimmed, topK: 10 },
      });
      if (!results) {
        toast.show("Semantic search requires Kronos desktop runtime", { variant: "error" });
        setChunks([]);
        return;
      }
      setChunks(results);
      setHasSearched(true);
    } catch (err) {
      toast.show(`Semantic search failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsSearching(false);
    }
  }, [query, cwd, toast]);

  const handleInsertChunk = useCallback(
    (chunk: SemanticCodeChunk) => {
      const sessions = useSessionStore.getState().sessions;
      const serverId = propServerId ?? Object.keys(sessions)[0];
      const activeAgentId =
        propAgentId ??
        (serverId ? Array.from(sessions[serverId]?.agents.values() ?? [])[0]?.id : undefined);
      const snippet = `[Semantic Match: ${chunk.relativePath}:${chunk.lineStart}-${chunk.lineEnd}]\n\`\`\`\n${chunk.content}\n\`\`\``;
      if (serverId && activeAgentId) {
        const draftKey = `agent:${serverId}:${activeAgentId}`;
        const existing = useDraftStore.getState().drafts[draftKey]?.input.text ?? "";
        useDraftStore.getState().replaceDraftText({
          draftKey,
          text: existing ? `${existing}\n\n${snippet}` : snippet,
        });
        toast.show(`Added ${chunk.relativePath} to prompt`, { variant: "success" });
        return;
      }
      if (navigator?.clipboard?.writeText) {
        void navigator.clipboard.writeText(snippet);
        toast.show("Copied semantic chunk to clipboard", { variant: "success" });
      }
    },
    [propAgentId, propServerId, toast],
  );

  const handleSearchPress = useCallback(() => {
    void handleSearch();
  }, [handleSearch]);

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedSearchIcon size={18} />
              <Text style={styles.title}>Semantic Codebase Search</Text>
            </View>
            <ToolbarButton label="Close" onPress={onClose} style={styles.closeBtn}>
              <ThemedCloseIcon size={18} />
            </ToolbarButton>
          </View>

          <View style={styles.searchBar}>
            <AdaptiveTextInput
              initialValue={query}
              onChangeText={setQuery}
              placeholder="Ask in natural language (e.g. how does agent checkpointing work?)"
              style={styles.input}
            />
            <Button size="sm" onPress={handleSearchPress} disabled={isSearching || !query.trim()}>
              <Search size={13} color="#ffffff" />
              <Text style={styles.btnText}>{isSearching ? "Indexing..." : "Search"}</Text>
            </Button>
          </View>

          <ScrollView style={styles.scroll} contentContainerStyle={styles.scrollContent}>
            {chunks.length === 0 && hasSearched ? (
              <Text style={styles.emptyText}>No matching code blocks found in {cwd}.</Text>
            ) : null}
            {chunks.length === 0 && !hasSearched ? (
              <Text style={styles.emptyText}>
                Search functions, components, and architecture concepts across {cwd}.
              </Text>
            ) : null}
            {chunks.map((chunk) => (
              <ChunkCard
                key={`${chunk.relativePath}:${chunk.lineStart}`}
                chunk={chunk}
                onInsert={handleInsertChunk}
              />
            ))}
          </ScrollView>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.65)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  card: {
    width: "100%",
    maxWidth: 780,
    height: "80%",
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
    padding: theme.spacing[4],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  titleRow: { flexDirection: "row", alignItems: "center", gap: 8 },
  title: { fontSize: theme.fontSize.base, fontWeight: "700", color: theme.colors.foreground },
  closeBtn: { padding: 4 },
  searchBar: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    padding: theme.spacing[4],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  input: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 6,
    paddingHorizontal: 10,
    paddingVertical: 8,
    color: theme.colors.foreground,
    fontSize: 13,
  },
  btnText: { color: "#ffffff", fontSize: 12, fontWeight: "600", marginLeft: 4 },
  scroll: { flex: 1 },
  scrollContent: { padding: theme.spacing[4], gap: 10 },
  emptyText: {
    textAlign: "center",
    color: theme.colors.foregroundMuted,
    fontSize: 13,
    paddingVertical: 32,
  },
  chunkCard: {
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 8,
    padding: 12,
    gap: 8,
  },
  chunkHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
  },
  chunkTitleRow: { flexDirection: "row", alignItems: "center", gap: 6, flex: 1 },
  chunkPath: {
    fontSize: 12,
    fontWeight: "600",
    color: theme.colors.foreground,
    fontFamily: theme.fontFamily.mono,
  },
  chunkActions: { flexDirection: "row", alignItems: "center", gap: 8 },
  scoreBadge: {
    fontSize: 11,
    fontWeight: "700",
    color: theme.colors.accent,
    fontFamily: theme.fontFamily.mono,
  },
  insertBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.accent,
    borderRadius: 6,
    paddingHorizontal: 8,
    paddingVertical: 4,
  },
  insertText: { fontSize: 11, fontWeight: "600", color: "#ffffff" },
  chunkCode: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    fontFamily: theme.fontFamily.mono,
    lineHeight: 16,
  },
}));
