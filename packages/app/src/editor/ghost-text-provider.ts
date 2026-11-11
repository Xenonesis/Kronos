import { useState, useCallback, useRef, useEffect } from "react";

export interface GhostTextSuggestion {
  id: string;
  prefix: string;
  suggestion: string;
  source: "heuristic" | "context" | "agent";
  confidence: number;
}

export interface UseGhostTextOptions {
  enabled?: boolean;
  debounceMs?: number;
  language?: string;
  onAccept?: (fullText: string) => void;
  onDismiss?: () => void;
}

function matchImportGhostText(currentLine: string, trimmed: string): GhostTextSuggestion | null {
  if (trimmed.startsWith("import {") && !trimmed.includes("from")) {
    const componentNameMatch = trimmed.match(/import\s*\{\s*([A-Za-z0-9_]+)/);
    if (componentNameMatch && !currentLine.endsWith("}")) {
      const name = componentNameMatch[1];
      return {
        id: `import-${Date.now()}`,
        prefix: currentLine,
        suggestion: ` } from "./${name.toLowerCase()}";`,
        source: "heuristic",
        confidence: 0.9,
      };
    }
  }
  return null;
}

function matchReactHookGhostText(currentLine: string, trimmed: string): GhostTextSuggestion | null {
  if (trimmed.startsWith("useEffect(") && !trimmed.includes("=>")) {
    return {
      id: `use-effect-${Date.now()}`,
      prefix: currentLine,
      suggestion: "() => {\n    \n  }, []);",
      source: "heuristic",
      confidence: 0.95,
    };
  }
  if (trimmed.startsWith("useCallback(") && !trimmed.includes("=>")) {
    return {
      id: `use-callback-${Date.now()}`,
      prefix: currentLine,
      suggestion: "() => {\n    \n  }, []);",
      source: "heuristic",
      confidence: 0.95,
    };
  }
  return null;
}

function matchControlFlowGhostText(
  currentLine: string,
  trimmed: string,
  textAfterCursor: string,
): GhostTextSuggestion | null {
  if (trimmed.includes("async ") && trimmed.endsWith("(") && !trimmed.includes(")")) {
    return {
      id: `async-fn-${Date.now()}`,
      prefix: currentLine,
      suggestion: ") => {\n    \n  };",
      source: "heuristic",
      confidence: 0.85,
    };
  }
  if (trimmed === "try {" || trimmed === "try") {
    return {
      id: `try-catch-${Date.now()}`,
      prefix: currentLine,
      suggestion: " {\n  } catch (error) {\n    console.error(error);\n  }",
      source: "heuristic",
      confidence: 0.9,
    };
  }
  if (trimmed.startsWith("if (") && trimmed.endsWith(")") && !textAfterCursor.startsWith(" {")) {
    return {
      id: `if-block-${Date.now()}`,
      prefix: currentLine,
      suggestion: " {\n    \n  }",
      source: "heuristic",
      confidence: 0.85,
    };
  }
  if (trimmed.startsWith("export interface ") && !trimmed.includes("{")) {
    return {
      id: `interface-${Date.now()}`,
      prefix: currentLine,
      suggestion: " {\n  id: string;\n}",
      source: "heuristic",
      confidence: 0.8,
    };
  }
  return null;
}

// Built-in intelligent completions for common idioms and syntax patterns
export function generatePredictiveGhostText(
  text: string,
  cursorPosition: number,
  _language = "typescript",
): GhostTextSuggestion | null {
  if (!text || cursorPosition < 2) return null;

  const textBeforeCursor = text.slice(0, cursorPosition);
  const textAfterCursor = text.slice(cursorPosition);
  const currentLine = textBeforeCursor.split("\n").pop() ?? "";
  const trimmed = currentLine.trim();

  const importMatch = matchImportGhostText(currentLine, trimmed);
  if (importMatch) return importMatch;

  const hookMatch = matchReactHookGhostText(currentLine, trimmed);
  if (hookMatch) return hookMatch;

  return matchControlFlowGhostText(currentLine, trimmed, textAfterCursor);
}

export function useGhostText(
  value: string,
  cursorPosition: number,
  options: UseGhostTextOptions = {},
) {
  const { enabled = true, debounceMs = 280, language = "typescript", onAccept, onDismiss } = options;
  const [suggestion, setSuggestion] = useState<GhostTextSuggestion | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!enabled) {
      setSuggestion(null);
      return;
    }

    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }

    timeoutRef.current = setTimeout(() => {
      const nextSuggestion = generatePredictiveGhostText(value, cursorPosition, language);
      setSuggestion(nextSuggestion);
    }, debounceMs);

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [value, cursorPosition, enabled, debounceMs, language]);

  const accept = useCallback(() => {
    if (!suggestion) return value;
    const textBefore = value.slice(0, cursorPosition);
    const textAfter = value.slice(cursorPosition);
    const completed = `${textBefore}${suggestion.suggestion}${textAfter}`;
    setSuggestion(null);
    onAccept?.(completed);
    return completed;
  }, [suggestion, value, cursorPosition, onAccept]);

  const dismiss = useCallback(() => {
    setSuggestion(null);
    onDismiss?.();
  }, [onDismiss]);

  return {
    suggestion,
    hasSuggestion: suggestion !== null,
    accept,
    dismiss,
  };
}
