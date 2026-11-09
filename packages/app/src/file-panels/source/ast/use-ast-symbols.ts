import { useEffect, useMemo, useRef, useState } from "react";
import { extractAstSymbols, findSymbolAtLine, flattenSymbols } from "./extract-symbols";
import type { AstSymbol } from "./types";

export interface UseAstSymbolsResult {
  symbols: AstSymbol[];
  flattenedSymbols: Array<AstSymbol & { displayName: string }>;
  activeSymbol: AstSymbol | null;
  loading: boolean;
}

export function useAstSymbols(
  content: string,
  filename: string,
  cursorLine?: number,
): UseAstSymbolsResult {
  const [symbols, setSymbols] = useState<AstSymbol[]>([]);
  const [loading, setLoading] = useState(false);
  const lastAnalyzed = useRef({ filename: "", content: "" });

  useEffect(() => {
    if (!content || !filename) {
      setSymbols([]);
      return;
    }

    if (lastAnalyzed.current.filename === filename && lastAnalyzed.current.content === content) {
      return;
    }

    let active = true;
    setLoading(true);

    const timer = setTimeout(() => {
      void extractAstSymbols(content, filename).then((extracted) => {
        if (!active) return;
        lastAnalyzed.current = { filename, content };
        setSymbols(extracted);
        setLoading(false);
      });
    }, 250);

    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [content, filename]);

  const flattenedSymbols = useMemo(() => flattenSymbols(symbols), [symbols]);

  const activeSymbol = useMemo(() => {
    if (!cursorLine || symbols.length === 0) return null;
    return findSymbolAtLine(symbols, cursorLine);
  }, [symbols, cursorLine]);

  return {
    symbols,
    flattenedSymbols,
    activeSymbol,
    loading,
  };
}
