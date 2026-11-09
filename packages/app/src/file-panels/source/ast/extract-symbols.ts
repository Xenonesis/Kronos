import type { AstAnalysisResult, AstSymbol, AstSymbolKind } from "./types";

/**
 * Traverses a symbol hierarchy to find the deepest enclosing symbol for a given 1-indexed line.
 */
export function findSymbolAtLine(symbols: AstSymbol[], line: number): AstSymbol | null {
  for (const sym of symbols) {
    if (sym.range.start_line <= line && line <= sym.range.end_line) {
      if (sym.children && sym.children.length > 0) {
        const childMatch = findSymbolAtLine(sym.children, line);
        if (childMatch) return childMatch;
      }
      return sym;
    }
  }
  return null;
}

/**
 * Flattens nested symbols (e.g. methods inside classes) into a list with qualified display names.
 */
export function flattenSymbols(
  symbols: AstSymbol[],
  parentName = "",
): Array<AstSymbol & { displayName: string }> {
  const result: Array<AstSymbol & { displayName: string }> = [];
  for (const sym of symbols) {
    const displayName = parentName ? `${parentName} > ${sym.name}` : sym.name;
    result.push({ ...sym, displayName });
    if (sym.children && sym.children.length > 0) {
      result.push(...flattenSymbols(sym.children, displayName));
    }
  }
  return result;
}

/**
 * Lightweight regex-based fallback symbol extractor for non-desktop environments or unsupported grammars.
 */
export function extractFallbackSymbols(content: string, filename: string): AstSymbol[] {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  const lines = content.split("\n");
  const symbols: AstSymbol[] = [];

  const patterns: Array<{ regex: RegExp; kind: AstSymbolKind; nameIndex: number }> = [];

  if (["ts", "tsx", "js", "jsx", "mjs", "cjs"].includes(ext)) {
    patterns.push(
      {
        regex: /^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z0-9_$]+)/,
        kind: "function",
        nameIndex: 1,
      },
      { regex: /^\s*(?:export\s+)?class\s+([A-Za-z0-9_$]+)/, kind: "class", nameIndex: 1 },
      { regex: /^\s*(?:export\s+)?interface\s+([A-Za-z0-9_$]+)/, kind: "interface", nameIndex: 1 },
      { regex: /^\s*(?:export\s+)?type\s+([A-Za-z0-9_$]+)\s*=/, kind: "type_alias", nameIndex: 1 },
      { regex: /^\s*(?:export\s+)?enum\s+([A-Za-z0-9_$]+)/, kind: "enum", nameIndex: 1 },
      {
        regex:
          /^\s*(?:export\s+)?(?:const|let)\s+([A-Za-z0-9_$]+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>/,
        kind: "arrow_function",
        nameIndex: 1,
      },
    );
  } else if (ext === "py") {
    patterns.push(
      { regex: /^\s*(?:async\s+)?def\s+([A-Za-z0-9_]+)/, kind: "function", nameIndex: 1 },
      { regex: /^\s*class\s+([A-Za-z0-9_]+)/, kind: "class", nameIndex: 1 },
    );
  } else if (ext === "rs") {
    patterns.push(
      {
        regex: /^\s*(?:pub(?:\([^)]+\))?\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)/,
        kind: "function",
        nameIndex: 1,
      },
      {
        regex: /^\s*(?:pub(?:\([^)]+\))?\s+)?struct\s+([A-Za-z0-9_]+)/,
        kind: "class",
        nameIndex: 1,
      },
      { regex: /^\s*(?:pub(?:\([^)]+\))?\s+)?enum\s+([A-Za-z0-9_]+)/, kind: "enum", nameIndex: 1 },
      { regex: /^\s*impl(?:\s*<[^>]+>)?\s+([A-Za-z0-9_]+)/, kind: "class", nameIndex: 1 },
    );
  } else if (ext === "go") {
    patterns.push(
      { regex: /^\s*func\s+(?:\([^)]+\)\s+)?([A-Za-z0-9_]+)/, kind: "function", nameIndex: 1 },
      { regex: /^\s*type\s+([A-Za-z0-9_]+)\s+struct/, kind: "class", nameIndex: 1 },
      { regex: /^\s*type\s+([A-Za-z0-9_]+)\s+interface/, kind: "interface", nameIndex: 1 },
    );
  }

  for (let i = 0; i < lines.length; i++) {
    const lineText = lines[i];
    const lineNum = i + 1;

    for (const pat of patterns) {
      const match = lineText.match(pat.regex);
      if (match && match[pat.nameIndex]) {
        const name = match[pat.nameIndex];
        const col = lineText.indexOf(name) + 1;
        symbols.push({
          name,
          kind: pat.kind,
          range: {
            start_line: lineNum,
            start_col: Math.max(1, col),
            end_line: lineNum,
            end_col: Math.max(1, col + name.length),
            start_byte: 0,
            end_byte: 0,
          },
          exported: lineText.includes("export") || lineText.includes("pub "),
          children: [],
        });
        break;
      }
    }
  }

  return symbols;
}

/**
 * Extracts AST symbols using the high-performance native Tree-Sitter kernel on desktop,
 * with instantaneous graceful fallback to regex inspection on web.
 */
export async function extractAstSymbols(content: string, filename: string): Promise<AstSymbol[]> {
  if (!content || !filename) return [];

  if (typeof window !== "undefined" && window.paseoDesktop?.invoke) {
    try {
      const res = (await window.paseoDesktop.invoke("analyze_ast", {
        source: content,
        path: filename,
      })) as AstAnalysisResult | null;

      if (res && Array.isArray(res.symbols) && res.symbols.length > 0) {
        return res.symbols;
      }
    } catch {
      // Fall through to regex parser
    }
  }

  return extractFallbackSymbols(content, filename);
}
