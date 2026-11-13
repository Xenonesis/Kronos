import React, { memo, useState, useCallback, useEffect, useMemo } from "react";
import {
  Search,
  Replace,
  ChevronDown,
  ChevronRight,
  FileCode,
  X,
  Check,
  CaseSensitive,
  WholeWord,
  Regex,
  Loader2,
} from "lucide-react-native";
import { useToast } from "@/contexts/toast-context";
import { invokeTauri } from "@/utils/tauri-invoke";

export interface SearchMatchItem {
  filePath: string;
  relativePath: string;
  lineNumber: number;
  lineText: string;
  matchStart: number;
  matchEnd: number;
}

export interface FileMatchGroup {
  relativePath: string;
  fullPath: string;
  matchCount: number;
  matches: SearchMatchItem[];
}

export interface GlobalSearchReplaceModalProps {
  visible: boolean;
  cwd: string;
  onOpenMatch?: (file: string, line: number) => void;
  onClose: () => void;
}

export const GlobalSearchReplaceModal = memo(function GlobalSearchReplaceModal({
  visible,
  cwd,
  onOpenMatch,
  onClose,
}: GlobalSearchReplaceModalProps) {
  const [query, setQuery] = useState("");
  const [replacement, setReplacement] = useState("");
  const [isCaseSensitive, setIsCaseSensitive] = useState(false);
  const [isWholeWord, setIsWholeWord] = useState(false);
  const [isRegex, setIsRegex] = useState(false);
  const [includeFilter, setIncludeFilter] = useState("");
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<FileMatchGroup[]>([]);
  const [expandedFiles, setExpandedFiles] = useState<Record<string, boolean>>({});
  const toast = useToast();

  const totalMatches = useMemo(
    () => results.reduce((acc, curr) => acc + curr.matchCount, 0),
    [results],
  );

  const toggleExpand = (path: string) => {
    setExpandedFiles((prev) => ({ ...prev, [path]: !prev[path] }));
  };

  const handleSearch = useCallback(async () => {
    if (!query.trim()) {
      setResults([]);
      return;
    }

    setIsSearching(true);
    try {
      const groups = await invokeTauri<FileMatchGroup[]>("workspace_text_search", {
        req: {
          cwd,
          query: query.trim(),
          isRegex,
          caseSensitive: isCaseSensitive,
          wholeWord: isWholeWord,
          includeFilter: includeFilter.trim() ? includeFilter.trim() : null,
          maxResults: 500,
        },
      });
      if (!groups) {
        toast.show("Workspace search requires Kronos desktop runtime", { variant: "error" });
        setResults([]);
        return;
      }
      setResults(groups);
      const expands: Record<string, boolean> = {};
      for (const g of groups) {
        expands[g.relativePath] = true;
      }
      setExpandedFiles(expands);
    } catch (err) {
      toast.show(`Search failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsSearching(false);
    }
  }, [query, cwd, isRegex, isCaseSensitive, isWholeWord, includeFilter, toast]);

  const handleBatchReplace = useCallback(async () => {
    if (!query.trim() || !results.length) return;

    try {
      const res = await invokeTauri<{
        filesModified: number;
        totalReplacements: number;
        modifiedPaths: string[];
      }>("workspace_batch_replace", {
        req: {
          cwd,
          query: query.trim(),
          replacement,
          isRegex,
          caseSensitive: isCaseSensitive,
          wholeWord: isWholeWord,
          includeFilter: includeFilter.trim() ? includeFilter.trim() : null,
        },
      });
      if (!res) {
        toast.show("Batch replace requires Kronos desktop runtime", { variant: "error" });
        return;
      }
      toast.show(
        `Replaced ${res.totalReplacements} occurrence(s) across ${res.filesModified} file(s)`,
        { variant: "success" },
      );
      setResults([]);
    } catch (err) {
      toast.show(`Batch replace failed: ${String(err)}`, { variant: "error" });
    }
  }, [
    query,
    results,
    cwd,
    replacement,
    isRegex,
    isCaseSensitive,
    isWholeWord,
    includeFilter,
    toast,
  ]);

  if (!visible) return null;

  return (
    <div
      data-testid="global-search-replace-modal"
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.7)",
        backdropFilter: "blur(4px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 9999,
        padding: "20px",
      }}
    >
      <div
        style={{
          width: "750px",
          maxWidth: "100%",
          maxHeight: "85vh",
          backgroundColor: "#18181b",
          border: "1px solid #27272a",
          borderRadius: "12px",
          display: "flex",
          flexDirection: "column",
          boxShadow: "0 20px 40px rgba(0, 0, 0, 0.6)",
          color: "#f4f4f5",
          fontFamily: "system-ui, -apple-system, sans-serif",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "14px 18px",
            borderBottom: "1px solid #27272a",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <Search size={16} color="#60a5fa" />
            <span style={{ fontWeight: 600, fontSize: "14px" }}>Search and Replace in Workspace</span>
          </div>
          <button
            onClick={onClose}
            style={{
              background: "transparent",
              border: "none",
              color: "#a1a1aa",
              cursor: "pointer",
              padding: "4px",
            }}
          >
            <X size={16} />
          </button>
        </div>

        {/* Search & Replace Inputs */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "10px",
            padding: "14px 18px",
            borderBottom: "1px solid #27272a",
            backgroundColor: "#141416",
          }}
        >
          {/* Query Row */}
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <div
              style={{
                display: "flex",
                flex: 1,
                alignItems: "center",
                backgroundColor: "#27272a",
                border: "1px solid #3f3f46",
                borderRadius: "6px",
                padding: "2px 8px",
              }}
            >
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && void handleSearch()}
                placeholder="Search across all files (Regex, Text)..."
                style={{
                  flex: 1,
                  backgroundColor: "transparent",
                  border: "none",
                  outline: "none",
                  color: "#f4f4f5",
                  fontSize: "13px",
                  padding: "6px 4px",
                }}
              />
              <div style={{ display: "flex", gap: "4px" }}>
                <button
                  onClick={() => setIsCaseSensitive((v) => !v)}
                  title="Match Case (Alt+C)"
                  style={{
                    backgroundColor: isCaseSensitive ? "#3b82f6" : "transparent",
                    color: isCaseSensitive ? "#fff" : "#a1a1aa",
                    border: "none",
                    borderRadius: "4px",
                    padding: "3px 6px",
                    cursor: "pointer",
                    fontSize: "11px",
                    fontWeight: "bold",
                  }}
                >
                  Aa
                </button>
                <button
                  onClick={() => setIsWholeWord((v) => !v)}
                  title="Match Whole Word (Alt+W)"
                  style={{
                    backgroundColor: isWholeWord ? "#3b82f6" : "transparent",
                    color: isWholeWord ? "#fff" : "#a1a1aa",
                    border: "none",
                    borderRadius: "4px",
                    padding: "3px 6px",
                    cursor: "pointer",
                    fontSize: "11px",
                    fontWeight: "bold",
                  }}
                >
                  |W|
                </button>
                <button
                  onClick={() => setIsRegex((v) => !v)}
                  title="Use Regular Expression (Alt+R)"
                  style={{
                    backgroundColor: isRegex ? "#3b82f6" : "transparent",
                    color: isRegex ? "#fff" : "#a1a1aa",
                    border: "none",
                    borderRadius: "4px",
                    padding: "3px 6px",
                    cursor: "pointer",
                    fontSize: "11px",
                    fontWeight: "bold",
                  }}
                >
                  .*
                </button>
              </div>
            </div>

            <button
              onClick={() => void handleSearch()}
              disabled={!query.trim() || isSearching}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                padding: "8px 14px",
                backgroundColor: query.trim() ? "#2563eb" : "#3f3f46",
                border: "none",
                borderRadius: "6px",
                color: "#fff",
                fontSize: "12px",
                fontWeight: 500,
                cursor: query.trim() && !isSearching ? "pointer" : "default",
              }}
            >
              {isSearching ? <Loader2 size={13} className="animate-spin" /> : <Search size={13} />}
              Search
            </button>
          </div>

          {/* Replace Row */}
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <div
              style={{
                display: "flex",
                flex: 1,
                alignItems: "center",
                backgroundColor: "#27272a",
                border: "1px solid #3f3f46",
                borderRadius: "6px",
                padding: "2px 8px",
              }}
            >
              <input
                value={replacement}
                onChange={(e) => setReplacement(e.target.value)}
                placeholder="Replace with..."
                style={{
                  flex: 1,
                  backgroundColor: "transparent",
                  border: "none",
                  outline: "none",
                  color: "#f4f4f5",
                  fontSize: "13px",
                  padding: "6px 4px",
                }}
              />
            </div>

            <button
              onClick={() => void handleBatchReplace()}
              disabled={!results.length || !query.trim()}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                padding: "8px 14px",
                backgroundColor: results.length ? "#16a34a" : "#3f3f46",
                border: "none",
                borderRadius: "6px",
                color: "#fff",
                fontSize: "12px",
                fontWeight: 500,
                cursor: results.length ? "pointer" : "default",
              }}
            >
              <Replace size={13} /> Replace All
            </button>
          </div>
        </div>

        {/* Results List */}
        <div style={{ flex: 1, overflowY: "auto", padding: "10px 18px", minHeight: "240px" }}>
          {results.length > 0 ? (
            <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
              <div style={{ fontSize: "12px", color: "#a1a1aa", marginBottom: "4px" }}>
                Found {totalMatches} results in {results.length} files
              </div>

              {results.map((group) => {
                const isExpanded = expandedFiles[group.relativePath];
                return (
                  <div
                    key={group.relativePath}
                    style={{
                      border: "1px solid #27272a",
                      borderRadius: "6px",
                      overflow: "hidden",
                    }}
                  >
                    {/* File Header */}
                    <div
                      onClick={() => toggleExpand(group.relativePath)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "6px",
                        padding: "6px 10px",
                        backgroundColor: "#27272a",
                        cursor: "pointer",
                        fontSize: "12px",
                        fontWeight: 500,
                      }}
                    >
                      {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                      <FileCode size={14} color="#60a5fa" />
                      <span style={{ flex: 1 }}>{group.relativePath}</span>
                      <span
                        style={{
                          backgroundColor: "#3f3f46",
                          padding: "1px 6px",
                          borderRadius: "10px",
                          fontSize: "11px",
                        }}
                      >
                        {group.matchCount}
                      </span>
                    </div>

                    {/* Matches */}
                    {isExpanded && (
                      <div style={{ display: "flex", flexDirection: "column" }}>
                        {group.matches.map((m, idx) => (
                          <div
                            key={idx}
                            onClick={() => {
                              onOpenMatch?.(m.filePath, m.lineNumber);
                              onClose();
                            }}
                            style={{
                              display: "flex",
                              alignItems: "center",
                              gap: "8px",
                              padding: "4px 14px",
                              borderTop: "1px solid #1f1f23",
                              fontFamily: "monospace",
                              fontSize: "12px",
                              backgroundColor: "#18181b",
                              cursor: "pointer",
                            }}
                          >
                            <span style={{ color: "#71717a", width: "32px", textAlign: "right" }}>
                              {m.lineNumber}
                            </span>
                            <span style={{ color: "#d4d4d8", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                              {m.lineText}
                            </span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          ) : (
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                height: "100%",
                color: "#71717a",
                gap: "8px",
                padding: "40px",
              }}
            >
              <Search size={32} />
              <div style={{ fontSize: "13px" }}>
                {query ? "No matches found" : "Enter a search term to find across files"}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
});
