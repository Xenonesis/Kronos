export type AstSymbolKind =
  | "function"
  | "async_function"
  | "generator_function"
  | "arrow_function"
  | "class"
  | "method"
  | "constructor"
  | "interface"
  | "type_alias"
  | "enum"
  | "enum_member"
  | "variable"
  | "constant"
  | "property"
  | "import"
  | "export"
  | "unknown";

export interface SourceRange {
  start_line: number; // 1-indexed
  start_col: number; // 1-indexed
  end_line: number; // 1-indexed
  end_col: number; // 1-indexed
  start_byte: number; // 0-indexed byte offset
  end_byte: number; // 0-indexed byte offset
}

export interface AstSymbol {
  name: string;
  kind: AstSymbolKind;
  range: SourceRange;
  exported: boolean;
  signature?: string;
  doc_comment?: string;
  children: AstSymbol[];
}

export interface AstAnalysisResult {
  root_kind: string;
  node_count: number;
  declarations: string[];
  symbols: AstSymbol[];
  imports: unknown[];
  errors: unknown[];
}
