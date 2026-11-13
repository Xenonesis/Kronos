import React, { memo, useState, useCallback, useEffect } from "react";
import {
  Globe,
  Search,
  FileText,
  Copy,
  ExternalLink,
  X,
  Sparkles,
  Loader2,
  Check,
} from "lucide-react-native";
import { useToast } from "@/contexts/toast-context";

export interface SearchResultItem {
  title: string;
  url: string;
  snippet: string;
}

export interface WebFetchItem {
  url: string;
  title?: string;
  contentMarkdown: string;
  characterCount: number;
  statusCode: number;
}

export interface WebSearchModalProps {
  visible: boolean;
  initialMode?: "search" | "fetch";
  initialQuery?: string;
  onClose: () => void;
  onInsertToComposer?: (text: string) => void;
}

export const WebSearchModal = memo(function WebSearchModal({
  visible,
  initialMode = "search",
  initialQuery = "",
  onClose,
  onInsertToComposer,
}: WebSearchModalProps) {
  const [mode, setMode] = useState<"search" | "fetch">(initialMode);
  const [query, setQuery] = useState(initialQuery);
  const [fetchUrl, setFetchUrl] = useState(initialQuery.startsWith("http") ? initialQuery : "");
  const [isLoading, setIsLoading] = useState(false);
  const [searchResults, setSearchResults] = useState<SearchResultItem[]>([]);
  const [fetchResult, setFetchResult] = useState<WebFetchItem | null>(null);
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const toast = useToast();

  useEffect(() => {
    if (visible) {
      setMode(initialMode);
      if (initialMode === "fetch" && initialQuery.startsWith("http")) {
        setFetchUrl(initialQuery);
      } else {
        setQuery(initialQuery);
      }
      setSearchResults([]);
      setFetchResult(null);
      setIsLoading(false);
      setCopiedIndex(null);
    }
  }, [visible, initialMode, initialQuery]);

  const handleSearch = useCallback(async () => {
    const q = query.trim();
    if (!q) return;

    setIsLoading(true);
    try {
      // Invoke Tauri command if available
      const tauri = (
        window as unknown as {
          __TAURI_INTERNALS__?: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      if (tauri && typeof tauri.invoke === "function") {
        const results = (await tauri.invoke("web_search_query", {
          query: q,
          limit: 8,
        })) as SearchResultItem[];
        setSearchResults(results);
        toast.show("Web search completed", { variant: "success" });
      } else {
        setSearchResults([]);
        toast.show("Live web search requires Kronos desktop runtime", { variant: "error" });
      }
    } catch (err) {
      toast.show(`Search failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsLoading(false);
    }
  }, [query, toast]);

  const handleFetch = useCallback(async () => {
    const targetUrl = fetchUrl.trim();
    if (!targetUrl) return;

    setIsLoading(true);
    try {
      const tauri = (
        window as unknown as {
          __TAURI_INTERNALS__?: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      if (tauri && typeof tauri.invoke === "function") {
        const result = (await tauri.invoke("web_scrape_url", {
          url: targetUrl,
          maxChars: 15000,
        })) as WebFetchItem;
        setFetchResult(result);
        toast.show("URL scraped successfully", { variant: "success" });
      } else {
        setFetchResult(null);
        toast.show("Live web scraping requires Kronos desktop runtime", { variant: "error" });
      }
    } catch (err) {
      toast.show(`Fetch failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsLoading(false);
    }
  }, [fetchUrl, toast]);

  const handleCopySnippet = useCallback(
    (text: string, index: number) => {
      if (navigator?.clipboard?.writeText) {
        navigator.clipboard.writeText(text);
        setCopiedIndex(index);
        setTimeout(() => setCopiedIndex(null), 2000);
        toast.show("Copied to clipboard", { variant: "success" });
      }
    },
    [toast],
  );

  const handleInsertResult = useCallback(
    (item: SearchResultItem) => {
      const formatted = `### [${item.title}](${item.url})\n${item.snippet}\n`;
      if (onInsertToComposer) {
        onInsertToComposer(formatted);
      } else if (navigator?.clipboard?.writeText) {
        navigator.clipboard.writeText(formatted);
      }
      toast.show("Search result inserted into composer", { variant: "success" });
      onClose();
    },
    [onInsertToComposer, onClose, toast],
  );

  const handleInsertFetchedMarkdown = useCallback(() => {
    if (!fetchResult) return;
    const formatted = `### Source: [${fetchResult.title || fetchResult.url}](${fetchResult.url})\n\n${fetchResult.contentMarkdown}\n`;
    if (onInsertToComposer) {
      onInsertToComposer(formatted);
    } else if (navigator?.clipboard?.writeText) {
      navigator.clipboard.writeText(formatted);
    }
    toast.show("Web Markdown inserted into composer", { variant: "success" });
    onClose();
  }, [fetchResult, onInsertToComposer, onClose, toast]);

  if (!visible) return null;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 9999,
        backgroundColor: "rgba(0, 0, 0, 0.75)",
        backdropFilter: "blur(4px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: "16px",
      }}
    >
      <div
        style={{
          width: "100%",
          maxWidth: "680px",
          maxHeight: "85vh",
          backgroundColor: "#18181b",
          border: "1px solid #27272a",
          borderRadius: "12px",
          display: "flex",
          flexDirection: "column",
          boxShadow: "0 25px 50px -12px rgba(0, 0, 0, 0.6)",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "14px 20px",
            borderBottom: "1px solid #27272a",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <div
              style={{
                width: "28px",
                height: "28px",
                borderRadius: "6px",
                backgroundColor: "rgba(59, 130, 246, 0.15)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Globe size={16} color="#60a5fa" />
            </div>
            <div>
              <div style={{ fontWeight: 600, fontSize: "14px", color: "#f4f4f5" }}>
                Kronos Live Web Engine
              </div>
              <div style={{ fontSize: "11px", color: "#71717a" }}>
                Zero-API-key web search & clean HTML-to-Markdown scraper
              </div>
            </div>
          </div>
          <button
            onClick={onClose}
            style={{
              background: "transparent",
              border: "none",
              color: "#a1a1aa",
              cursor: "pointer",
              padding: "4px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <X size={18} />
          </button>
        </div>

        {/* Tab Switcher */}
        <div
          style={{
            display: "flex",
            padding: "8px 20px",
            borderBottom: "1px solid #27272a",
            gap: "8px",
            backgroundColor: "#121214",
          }}
        >
          <button
            onClick={() => setMode("search")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 12px",
              borderRadius: "6px",
              fontSize: "12px",
              fontWeight: 500,
              backgroundColor: mode === "search" ? "#27272a" : "transparent",
              color: mode === "search" ? "#60a5fa" : "#a1a1aa",
              border: "none",
              cursor: "pointer",
            }}
          >
            <Search size={14} /> Search Web (/web)
          </button>
          <button
            onClick={() => setMode("fetch")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 12px",
              borderRadius: "6px",
              fontSize: "12px",
              fontWeight: 500,
              backgroundColor: mode === "fetch" ? "#27272a" : "transparent",
              color: mode === "fetch" ? "#60a5fa" : "#a1a1aa",
              border: "none",
              cursor: "pointer",
            }}
          >
            <FileText size={14} /> Scrape & Convert URL (/fetch)
          </button>
        </div>

        {/* Content Body */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "14px",
            padding: "20px",
            overflowY: "auto",
            flex: 1,
          }}
        >
          {mode === "search" ? (
            <>
              {/* Search input bar */}
              <div style={{ display: "flex", gap: "8px" }}>
                <input
                  type="text"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSearch();
                  }}
                  placeholder="Enter search query (e.g. NVIDIA news, React 19 hooks, Rust axum)..."
                  style={{
                    flex: 1,
                    backgroundColor: "#131315",
                    border: "1px solid #3f3f46",
                    borderRadius: "6px",
                    padding: "8px 12px",
                    color: "#f4f4f5",
                    fontSize: "13px",
                    outline: "none",
                  }}
                />
                <button
                  onClick={handleSearch}
                  disabled={isLoading || !query.trim()}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "6px",
                    padding: "8px 16px",
                    backgroundColor: "#2563eb",
                    color: "#fff",
                    fontWeight: 500,
                    fontSize: "13px",
                    borderRadius: "6px",
                    border: "none",
                    cursor: isLoading || !query.trim() ? "not-allowed" : "pointer",
                    opacity: isLoading || !query.trim() ? 0.6 : 1,
                  }}
                >
                  {isLoading ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <Search size={14} />
                  )}
                  Search
                </button>
              </div>

              {/* Search Results list */}
              {searchResults.length > 0 ? (
                <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                  <div style={{ fontSize: "12px", color: "#a1a1aa" }}>
                    Top Results ({searchResults.length}):
                  </div>
                  {searchResults.map((item, idx) => (
                    <div
                      key={idx}
                      style={{
                        padding: "12px",
                        backgroundColor: "#131315",
                        border: "1px solid #27272a",
                        borderRadius: "8px",
                        display: "flex",
                        flexDirection: "column",
                        gap: "6px",
                      }}
                    >
                      <div
                        style={{
                          display: "flex",
                          justifyContent: "space-between",
                          alignItems: "flex-start",
                          gap: "8px",
                        }}
                      >
                        <a
                          href={item.url}
                          target="_blank"
                          rel="noreferrer"
                          style={{
                            fontSize: "13px",
                            fontWeight: 600,
                            color: "#60a5fa",
                            textDecoration: "none",
                            display: "flex",
                            alignItems: "center",
                            gap: "4px",
                          }}
                        >
                          {item.title} <ExternalLink size={12} />
                        </a>
                        <div style={{ display: "flex", gap: "6px" }}>
                          <button
                            onClick={() => handleCopySnippet(item.snippet, idx)}
                            title="Copy snippet"
                            style={{
                              background: "transparent",
                              border: "1px solid #3f3f46",
                              borderRadius: "4px",
                              padding: "3px 6px",
                              color: "#a1a1aa",
                              cursor: "pointer",
                              display: "flex",
                              alignItems: "center",
                              gap: "4px",
                              fontSize: "11px",
                            }}
                          >
                            {copiedIndex === idx ? (
                              <Check size={12} color="#22c55e" />
                            ) : (
                              <Copy size={12} />
                            )}
                          </button>
                          <button
                            onClick={() => handleInsertResult(item)}
                            title="Insert into composer"
                            style={{
                              backgroundColor: "rgba(59, 130, 246, 0.15)",
                              border: "1px solid rgba(59, 130, 246, 0.3)",
                              borderRadius: "4px",
                              padding: "3px 8px",
                              color: "#93c5fd",
                              cursor: "pointer",
                              display: "flex",
                              alignItems: "center",
                              gap: "4px",
                              fontSize: "11px",
                              fontWeight: 500,
                            }}
                          >
                            <Sparkles size={12} /> Insert
                          </button>
                        </div>
                      </div>
                      <div style={{ fontSize: "11px", color: "#71717a", wordBreak: "break-all" }}>
                        {item.url}
                      </div>
                      <div style={{ fontSize: "12px", color: "#d4d4d8", lineHeight: 1.5 }}>
                        {item.snippet}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                !isLoading && (
                  <div
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      justifyContent: "center",
                      padding: "40px",
                      color: "#71717a",
                      gap: "8px",
                    }}
                  >
                    <Globe size={32} />
                    <div style={{ fontSize: "13px" }}>
                      Search the live internet directly without third-party API keys
                    </div>
                  </div>
                )
              )}
            </>
          ) : (
            <>
              {/* URL scrape bar */}
              <div style={{ display: "flex", gap: "8px" }}>
                <input
                  type="text"
                  value={fetchUrl}
                  onChange={(e) => setFetchUrl(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleFetch();
                  }}
                  placeholder="https://docs.rs/tokio/latest/tokio/ or any documentation URL..."
                  style={{
                    flex: 1,
                    backgroundColor: "#131315",
                    border: "1px solid #3f3f46",
                    borderRadius: "6px",
                    padding: "8px 12px",
                    color: "#f4f4f5",
                    fontSize: "13px",
                    outline: "none",
                  }}
                />
                <button
                  onClick={handleFetch}
                  disabled={isLoading || !fetchUrl.trim()}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "6px",
                    padding: "8px 16px",
                    backgroundColor: "#2563eb",
                    color: "#fff",
                    fontWeight: 500,
                    fontSize: "13px",
                    borderRadius: "6px",
                    border: "none",
                    cursor: isLoading || !fetchUrl.trim() ? "not-allowed" : "pointer",
                    opacity: isLoading || !fetchUrl.trim() ? 0.6 : 1,
                  }}
                >
                  {isLoading ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <FileText size={14} />
                  )}
                  Scrape & Convert
                </button>
              </div>

              {/* Scrape Result View */}
              {fetchResult ? (
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "10px",
                    padding: "16px",
                    backgroundColor: "#131315",
                    border: "1px solid #27272a",
                    borderRadius: "8px",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                    }}
                  >
                    <div>
                      <div style={{ fontWeight: 600, fontSize: "13px", color: "#f4f4f5" }}>
                        {fetchResult.title || "Page Content"}
                      </div>
                      <div style={{ fontSize: "11px", color: "#71717a" }}>
                        {fetchResult.characterCount} characters • HTTP {fetchResult.statusCode}
                      </div>
                    </div>
                    <button
                      onClick={handleInsertFetchedMarkdown}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "6px",
                        padding: "6px 12px",
                        backgroundColor: "#2563eb",
                        color: "#fff",
                        borderRadius: "6px",
                        border: "none",
                        fontSize: "12px",
                        fontWeight: 500,
                        cursor: "pointer",
                      }}
                    >
                      <Sparkles size={13} /> Insert Markdown into Composer
                    </button>
                  </div>
                  <pre
                    style={{
                      maxHeight: "260px",
                      overflowY: "auto",
                      backgroundColor: "#09090b",
                      border: "1px solid #27272a",
                      borderRadius: "6px",
                      padding: "12px",
                      fontSize: "12px",
                      color: "#e4e4e7",
                      whiteSpace: "pre-wrap",
                      fontFamily: "monospace",
                    }}
                  >
                    {fetchResult.contentMarkdown}
                  </pre>
                </div>
              ) : (
                !isLoading && (
                  <div
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      justifyContent: "center",
                      padding: "40px",
                      color: "#71717a",
                      gap: "8px",
                    }}
                  >
                    <FileText size={32} />
                    <div style={{ fontSize: "13px" }}>
                      Enter any webpage URL to convert it to clean Markdown without ads or scripts
                    </div>
                  </div>
                )
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            alignItems: "center",
            padding: "12px 20px",
            borderTop: "1px solid #27272a",
          }}
        >
          <button
            onClick={onClose}
            style={{
              padding: "6px 14px",
              backgroundColor: "transparent",
              border: "1px solid #3f3f46",
              borderRadius: "6px",
              color: "#d4d4d8",
              fontSize: "13px",
              cursor: "pointer",
            }}
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
});
