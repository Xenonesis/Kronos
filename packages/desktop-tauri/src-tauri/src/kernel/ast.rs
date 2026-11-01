use std::path::Path;
use tree_sitter::{Node, Parser};

/// Detailed position and byte ranges in source text.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SourceRange {
    pub start_line: usize, // 1-indexed
    pub start_col: usize,  // 1-indexed
    pub end_line: usize,   // 1-indexed
    pub end_col: usize,    // 1-indexed
    pub start_byte: usize, // 0-indexed byte offset
    pub end_byte: usize,   // 0-indexed byte offset
}

impl SourceRange {
    /// Constructs a `SourceRange` from a Tree-Sitter `Node`.
    pub fn from_node(node: Node) -> Self {
        let start_pos = node.start_position();
        let end_pos = node.end_position();
        Self {
            start_line: start_pos.row + 1,
            start_col: start_pos.column + 1,
            end_line: end_pos.row + 1,
            end_col: end_pos.column + 1,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        }
    }

    /// Returns `true` if the given 1-indexed line and column falls within this range.
    pub fn contains_position(&self, line: usize, col: usize) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col > self.end_col {
            return false;
        }
        true
    }
}

/// Kinds of code symbols extracted from the AST.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AstSymbolKind {
    Function,
    AsyncFunction,
    GeneratorFunction,
    ArrowFunction,
    Class,
    Method,
    Constructor,
    Interface,
    TypeAlias,
    Enum,
    EnumMember,
    Variable,
    Constant,
    Property,
    Import,
    Export,
    Unknown,
}

/// Rich semantic symbol model representing functions, classes, interfaces, and variables.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AstSymbol {
    pub name: String,
    pub kind: AstSymbolKind,
    pub range: SourceRange,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_comment: Option<String>,
    pub children: Vec<AstSymbol>,
}

/// Imported module or binding extracted from the AST.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AstImport {
    pub source: String,
    pub specifiers: Vec<String>,
    pub is_default: bool,
    pub is_type_only: bool,
    pub range: SourceRange,
}

/// Syntax error or missing node discovered in AST.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AstSyntaxError {
    pub message: String,
    pub range: SourceRange,
}

/// Complete AST analysis report containing summary, symbols, imports, and syntax errors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AstAnalysisResult {
    pub root_kind: String,
    pub node_count: usize,
    pub declarations: Vec<String>,
    pub symbols: Vec<AstSymbol>,
    pub imports: Vec<AstImport>,
    pub errors: Vec<AstSyntaxError>,
}

/// Summary of an Abstract Syntax Tree (AST) parsed by Tree-Sitter (backwards-compatible).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AstSummary {
    pub root_kind: String,
    pub node_count: usize,
    pub declarations: Vec<String>,
}

/// Native Tree-Sitter AST Engine for fast syntax inspection, symbol extraction, and semantic diffing.
#[derive(Debug, Clone, Default)]
pub struct AstEngine;

impl AstEngine {
    /// Creates a new `AstEngine` instance.
    pub fn new() -> Self {
        Self
    }

    /// Determines the Tree-Sitter `Language` based on file extension.
    fn get_language_for_ext(ext: &str) -> Option<tree_sitter::Language> {
        let clean_ext = ext.trim_start_matches('.').to_lowercase();
        match clean_ext.as_str() {
            "ts" | "mts" | "cts" | "js" | "mjs" | "cjs" => {
                Some(tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT))
            }
            "tsx" | "jsx" => {
                Some(tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TSX))
            }
            _ => None,
        }
    }

    /// Parses the syntax of the provided source code based on file extension.
    /// Provides backwards compatibility returning `AstSummary`.
    pub fn parse_syntax(&self, source: &str, ext: &str) -> Result<AstSummary, String> {
        let language = match Self::get_language_for_ext(ext) {
            Some(lang) => lang,
            None => {
                return Ok(AstSummary {
                    root_kind: "unsupported".to_string(),
                    node_count: 0,
                    declarations: Vec::new(),
                });
            }
        };

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set Tree-Sitter language: {e}"))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| "Failed to parse syntax tree".to_string())?;

        let root = tree.root_node();
        let root_kind = root.kind().to_string();
        let node_count = count_nodes(root);
        let declarations = extract_declarations(root, source.as_bytes());

        Ok(AstSummary {
            root_kind,
            node_count,
            declarations,
        })
    }

    /// Performs deep semantic analysis on the source code, extracting structured symbols,
    /// imports, declarations, and syntax errors.
    pub fn analyze(&self, source: &str, ext: &str) -> Result<AstAnalysisResult, String> {
        let language = match Self::get_language_for_ext(ext) {
            Some(lang) => lang,
            None => {
                return Ok(AstAnalysisResult {
                    root_kind: "unsupported".to_string(),
                    node_count: 0,
                    declarations: Vec::new(),
                    symbols: Vec::new(),
                    imports: Vec::new(),
                    errors: Vec::new(),
                });
            }
        };

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set Tree-Sitter language: {e}"))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| "Failed to parse syntax tree".to_string())?;

        let root = tree.root_node();
        let source_bytes = source.as_bytes();
        let root_kind = root.kind().to_string();
        let node_count = count_nodes(root);
        let declarations = extract_declarations(root, source_bytes);
        let symbols = extract_rich_symbols(root, source_bytes);
        let imports = extract_imports(root, source_bytes);
        let errors = collect_syntax_errors(root, source_bytes);

        Ok(AstAnalysisResult {
            root_kind,
            node_count,
            declarations,
            symbols,
            imports,
            errors,
        })
    }

    /// Extracts all semantic symbols defined in the source file.
    pub fn extract_symbols(&self, source: &str, ext: &str) -> Result<Vec<AstSymbol>, String> {
        let result = self.analyze(source, ext)?;
        Ok(result.symbols)
    }

    /// Extracts all import statements from the source file.
    pub fn extract_imports(&self, source: &str, ext: &str) -> Result<Vec<AstImport>, String> {
        let result = self.analyze(source, ext)?;
        Ok(result.imports)
    }

    /// Finds the deepest enclosing symbol that contains the given 1-indexed line and column.
    pub fn find_symbol_at_position(
        &self,
        source: &str,
        ext: &str,
        line: usize,
        col: usize,
    ) -> Result<Option<AstSymbol>, String> {
        let symbols = self.extract_symbols(source, ext)?;
        Ok(find_deepest_symbol(&symbols, line, col))
    }

    /// Scans source text and returns any syntax errors.
    pub fn detect_errors(&self, source: &str, ext: &str) -> Result<Vec<AstSyntaxError>, String> {
        let result = self.analyze(source, ext)?;
        Ok(result.errors)
    }

    /// Helper to read a file from disk and perform analysis based on its file extension.
    pub fn analyze_file(&self, path: impl AsRef<Path>) -> Result<AstAnalysisResult, String> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref)
            .map_err(|e| format!("Failed to read file '{}': {e}", path_ref.display()))?;
        let ext = path_ref
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("ts");
        self.analyze(&content, ext)
    }
}

/// Recursively counts all AST nodes starting from `root`.
fn count_nodes(root: Node) -> usize {
    let mut count = 0;
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        count += 1;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    count
}

/// Searches recursively for the deepest symbol containing `(line, col)`.
fn find_deepest_symbol(symbols: &[AstSymbol], line: usize, col: usize) -> Option<AstSymbol> {
    for sym in symbols {
        if sym.range.contains_position(line, col) {
            if let Some(child_match) = find_deepest_symbol(&sym.children, line, col) {
                return Some(child_match);
            }
            return Some(sym.clone());
        }
    }
    None
}

/// Traverses tree and identifies syntax errors and missing nodes.
fn collect_syntax_errors(root: Node, source_bytes: &[u8]) -> Vec<AstSyntaxError> {
    let mut errors = Vec::new();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if node.is_error() || node.kind() == "ERROR" {
            let snippet = node
                .utf8_text(source_bytes)
                .unwrap_or("<invalid utf-8>")
                .chars()
                .take(40)
                .collect::<String>();
            errors.push(AstSyntaxError {
                message: format!("Syntax error near: '{snippet}'"),
                range: SourceRange::from_node(node),
            });
        } else if node.is_missing() {
            errors.push(AstSyntaxError {
                message: format!("Missing syntax token: '{}'", node.kind()),
                range: SourceRange::from_node(node),
            });
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    errors
}

/// Extracts doc comment (JSDoc `/** ... */` or `/// ...`) preceding a node.
fn extract_doc_comment(node: Node, source_bytes: &[u8]) -> Option<String> {
    let check_node = if let Some(parent) = node.parent() {
        if parent.kind() == "export_statement" {
            parent
        } else {
            node
        }
    } else {
        node
    };

    let mut prev = check_node.prev_sibling();
    while let Some(p) = prev {
        if p.kind() == "comment" {
            if let Ok(text) = p.utf8_text(source_bytes) {
                let trimmed = text.trim();
                if trimmed.starts_with("/**") || trimmed.starts_with("///") {
                    return Some(trimmed.to_string());
                }
            }
            prev = p.prev_sibling();
        } else {
            break;
        }
    }
    None
}

/// Resolves the effective range of a symbol, extending to the enclosing export statement if exported.
fn get_effective_range(node: Node, is_exported: bool) -> SourceRange {
    if is_exported {
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return SourceRange::from_node(parent);
            }
        }
    }
    SourceRange::from_node(node)
}

/// Builds signature string from function or method node.
fn extract_signature(node: Node, source_bytes: &[u8]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    // Check async
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "async" {
            parts.push("async".to_string());
            break;
        }
    }

    let kind_label = match node.kind() {
        "function_declaration" => "function",
        "generator_function_declaration" => "function*",
        "method_definition" => "",
        "class_declaration" => "class",
        "interface_declaration" => "interface",
        "type_alias_declaration" => "type",
        "enum_declaration" => "enum",
        _ => "",
    };
    if !kind_label.is_empty() {
        parts.push(kind_label.to_string());
    }

    if let Some(name_node) = node.child_by_field_name("name") {
        if let Ok(name) = name_node.utf8_text(source_bytes) {
            parts.push(name.to_string());
        }
    }

    if let Some(params_node) = node.child_by_field_name("parameters") {
        if let Ok(params) = params_node.utf8_text(source_bytes) {
            let single_line_params = params.replace('\n', " ").split_whitespace().collect::<Vec<_>>().join(" ");
            parts.push(single_line_params);
        }
    }

    if let Some(ret_node) = node.child_by_field_name("return_type") {
        if let Ok(ret) = ret_node.utf8_text(source_bytes) {
            let single_line_ret = ret.replace('\n', " ").split_whitespace().collect::<Vec<_>>().join(" ");
            parts.push(single_line_ret);
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Extracts rich semantic symbols from top-level and nested structure.
fn extract_rich_symbols(root: Node, source_bytes: &[u8]) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        collect_symbol_from_node(child, source_bytes, false, &mut symbols);
    }

    symbols
}

/// Recursively collects symbols from AST statements.
fn collect_symbol_from_node(
    node: Node,
    source_bytes: &[u8],
    is_exported: bool,
    symbols: &mut Vec<AstSymbol>,
) {
    match node.kind() {
        "export_statement" => {
            if let Some(decl) = node.child_by_field_name("declaration") {
                collect_symbol_from_node(decl, source_bytes, true, symbols);
            } else {
                let mut cursor = node.walk();
                for inner in node.children(&mut cursor) {
                    if inner.kind() != "export" && inner.kind() != "default" && inner.kind() != ";" {
                        collect_symbol_from_node(inner, source_bytes, true, symbols);
                    }
                }
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
                .unwrap_or("<anonymous>")
                .to_string();

            let is_async = has_child_kind(node, "async");
            let kind = if node.kind() == "generator_function_declaration" {
                AstSymbolKind::GeneratorFunction
            } else if is_async {
                AstSymbolKind::AsyncFunction
            } else {
                AstSymbolKind::Function
            };

            let range = get_effective_range(node, is_exported);
            let signature = extract_signature(node, source_bytes);
            let doc_comment = extract_doc_comment(node, source_bytes);

            symbols.push(AstSymbol {
                name,
                kind,
                range,
                exported: is_exported,
                signature,
                doc_comment,
                children: Vec::new(),
            });
        }
        "class_declaration" => {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
                .unwrap_or("<anonymous>")
                .to_string();

            let range = get_effective_range(node, is_exported);
            let signature = extract_signature(node, source_bytes);
            let doc_comment = extract_doc_comment(node, source_bytes);

            let mut class_children = Vec::new();
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for member in body.children(&mut cursor) {
                    collect_class_member(member, source_bytes, &mut class_children);
                }
            }

            symbols.push(AstSymbol {
                name,
                kind: AstSymbolKind::Class,
                range,
                exported: is_exported,
                signature,
                doc_comment,
                children: class_children,
            });
        }
        "interface_declaration" => {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
                .unwrap_or("<anonymous>")
                .to_string();

            let range = get_effective_range(node, is_exported);
            let signature = extract_signature(node, source_bytes);
            let doc_comment = extract_doc_comment(node, source_bytes);

            let mut interface_children = Vec::new();
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for member in body.children(&mut cursor) {
                    collect_interface_member(member, source_bytes, &mut interface_children);
                }
            }

            symbols.push(AstSymbol {
                name,
                kind: AstSymbolKind::Interface,
                range,
                exported: is_exported,
                signature,
                doc_comment,
                children: interface_children,
            });
        }
        "type_alias_declaration" => {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
                .unwrap_or("<anonymous>")
                .to_string();

            let range = get_effective_range(node, is_exported);
            let signature = extract_signature(node, source_bytes);
            let doc_comment = extract_doc_comment(node, source_bytes);

            symbols.push(AstSymbol {
                name,
                kind: AstSymbolKind::TypeAlias,
                range,
                exported: is_exported,
                signature,
                doc_comment,
                children: Vec::new(),
            });
        }
        "enum_declaration" => {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
                .unwrap_or("<anonymous>")
                .to_string();

            let range = get_effective_range(node, is_exported);
            let signature = extract_signature(node, source_bytes);
            let doc_comment = extract_doc_comment(node, source_bytes);

            let mut enum_children = Vec::new();
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for member in body.children(&mut cursor) {
                    if member.kind() == "property_identifier" || member.kind() == "enum_assignment" {
                        let member_name = if member.kind() == "enum_assignment" {
                            member
                                .child_by_field_name("name")
                                .and_then(|n| n.utf8_text(source_bytes).ok())
                                .unwrap_or("<member>")
                        } else {
                            member.utf8_text(source_bytes).unwrap_or("<member>")
                        };
                        enum_children.push(AstSymbol {
                            name: member_name.to_string(),
                            kind: AstSymbolKind::EnumMember,
                            range: SourceRange::from_node(member),
                            exported: false,
                            signature: None,
                            doc_comment: None,
                            children: Vec::new(),
                        });
                    }
                }
            }

            symbols.push(AstSymbol {
                name,
                kind: AstSymbolKind::Enum,
                range,
                exported: is_exported,
                signature,
                doc_comment,
                children: enum_children,
            });
        }
        "lexical_declaration" | "variable_declaration" => {
            let is_const = has_child_kind(node, "const");
            let doc_comment = extract_doc_comment(node, source_bytes);

            let mut cursor = node.walk();
            for declarator in node.children(&mut cursor) {
                if declarator.kind() == "variable_declarator" {
                    if let Some(name_node) = declarator.child_by_field_name("name") {
                        let range = if is_exported { get_effective_range(node, is_exported) } else { SourceRange::from_node(declarator) };

                        let mut kind = if is_const {
                            AstSymbolKind::Constant
                        } else {
                            AstSymbolKind::Variable
                        };

                        // Check if value is an arrow function
                        if let Some(val_node) = declarator.child_by_field_name("value") {
                            if val_node.kind() == "arrow_function" {
                                if has_child_kind(val_node, "async") {
                                    kind = AstSymbolKind::AsyncFunction;
                                } else {
                                    kind = AstSymbolKind::ArrowFunction;
                                }
                            }
                        }

                        if name_node.kind() == "identifier" {
                            if let Ok(name) = name_node.utf8_text(source_bytes) {
                                symbols.push(AstSymbol {
                                    name: name.to_string(),
                                    kind,
                                    range,
                                    exported: is_exported,
                                    signature: None,
                                    doc_comment: doc_comment.clone(),
                                    children: Vec::new(),
                                });
                            }
                        } else {
                            let mut pattern_names = Vec::new();
                            collect_names_from_pattern(name_node, source_bytes, &mut pattern_names);
                            for name in pattern_names {
                                symbols.push(AstSymbol {
                                    name,
                                    kind,
                                    range,
                                    exported: is_exported,
                                    signature: None,
                                    doc_comment: doc_comment.clone(),
                                    children: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Collects member functions and fields from a class body.
fn collect_class_member(member: Node, source_bytes: &[u8], children: &mut Vec<AstSymbol>) {
    match member.kind() {
        "method_definition" => {
            let name = member
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
                .unwrap_or("<method>")
                .to_string();

            let kind = if name == "constructor" {
                AstSymbolKind::Constructor
            } else {
                AstSymbolKind::Method
            };

            let range = SourceRange::from_node(member);
            let signature = extract_signature(member, source_bytes);
            let doc_comment = extract_doc_comment(member, source_bytes);

            children.push(AstSymbol {
                name,
                kind,
                range,
                exported: false,
                signature,
                doc_comment,
                children: Vec::new(),
            });
        }
        "public_field_definition" | "field_definition" | "property_definition" => {
            if let Some(name_node) = member.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    children.push(AstSymbol {
                        name: name.to_string(),
                        kind: AstSymbolKind::Property,
                        range: SourceRange::from_node(member),
                        exported: false,
                        signature: None,
                        doc_comment: extract_doc_comment(member, source_bytes),
                        children: Vec::new(),
                    });
                }
            }
        }
        _ => {}
    }
}

/// Collects property and method signatures from an interface body.
fn collect_interface_member(member: Node, source_bytes: &[u8], children: &mut Vec<AstSymbol>) {
    match member.kind() {
        "property_signature" => {
            if let Some(name_node) = member.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    children.push(AstSymbol {
                        name: name.to_string(),
                        kind: AstSymbolKind::Property,
                        range: SourceRange::from_node(member),
                        exported: false,
                        signature: None,
                        doc_comment: extract_doc_comment(member, source_bytes),
                        children: Vec::new(),
                    });
                }
            }
        }
        "method_signature" => {
            if let Some(name_node) = member.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    children.push(AstSymbol {
                        name: name.to_string(),
                        kind: AstSymbolKind::Method,
                        range: SourceRange::from_node(member),
                        exported: false,
                        signature: extract_signature(member, source_bytes),
                        doc_comment: extract_doc_comment(member, source_bytes),
                        children: Vec::new(),
                    });
                }
            }
        }
        _ => {}
    }
}

/// Extracts all import statements and specifiers.
fn extract_imports(root: Node, source_bytes: &[u8]) -> Vec<AstImport> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_statement" {
            let range = SourceRange::from_node(child);
            let mut source_path = String::new();
            let mut specifiers = Vec::new();
            let mut is_default = false;
            let mut is_type_only = false;

            if let Some(source_node) = child.child_by_field_name("source") {
                if let Ok(raw_source) = source_node.utf8_text(source_bytes) {
                    source_path = raw_source.trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string();
                }
            }

            let mut sub_cursor = child.walk();
            for sub in child.children(&mut sub_cursor) {
                if sub.kind() == "type" {
                    is_type_only = true;
                } else if sub.kind() == "import_clause" {
                    let mut clause_cursor = sub.walk();
                    for clause_child in sub.children(&mut clause_cursor) {
                        if clause_child.kind() == "identifier" {
                            // Default import
                            if let Ok(name) = clause_child.utf8_text(source_bytes) {
                                specifiers.push(name.to_string());
                                is_default = true;
                            }
                        } else if clause_child.kind() == "named_imports" {
                            let mut named_cursor = clause_child.walk();
                            for spec in clause_child.children(&mut named_cursor) {
                                if spec.kind() == "import_specifier" {
                                    if let Some(name_node) = spec.child_by_field_name("name") {
                                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                                            specifiers.push(name.to_string());
                                        }
                                    }
                                }
                            }
                        } else if clause_child.kind() == "namespace_import" {
                            if let Ok(raw) = clause_child.utf8_text(source_bytes) {
                                specifiers.push(raw.to_string());
                            }
                        }
                    }
                }
            }

            imports.push(AstImport {
                source: source_path,
                specifiers,
                is_default,
                is_type_only,
                range,
            });
        }
    }

    imports
}

/// Helper to check if a node has any immediate child of a specified kind.
fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return true;
        }
    }
    false
}

/// Extracts top-level declarations (functions, classes, interfaces, variables)
/// from the root node of a TypeScript/JavaScript syntax tree.
fn extract_declarations(root: Node, source_bytes: &[u8]) -> Vec<String> {
    let mut declarations = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        collect_declarations_from_node(child, source_bytes, &mut declarations);
    }

    declarations
}

/// Collects declarations from a given top-level node or an exported statement.
fn collect_declarations_from_node(node: Node, source_bytes: &[u8], declarations: &mut Vec<String>) {
    match node.kind() {
        "export_statement" => {
            if let Some(decl) = node.child_by_field_name("declaration") {
                collect_declarations_from_node(decl, source_bytes, declarations);
            } else {
                let mut cursor = node.walk();
                for inner in node.children(&mut cursor) {
                    if inner.kind() != "export" && inner.kind() != "default" {
                        collect_declarations_from_node(inner, source_bytes, declarations);
                    }
                }
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    declarations.push(name.to_string());
                }
            }
        }
        "class_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    declarations.push(name.to_string());
                }
            }
        }
        "interface_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    declarations.push(name.to_string());
                }
            }
        }
        "type_alias_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    declarations.push(name.to_string());
                }
            }
        }
        "enum_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    declarations.push(name.to_string());
                }
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            let mut cursor = node.walk();
            for declarator in node.children(&mut cursor) {
                if declarator.kind() == "variable_declarator" {
                    if let Some(name_node) = declarator.child_by_field_name("name") {
                        collect_names_from_pattern(name_node, source_bytes, declarations);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Collects variable or property names from an identifier or destructuring pattern.
fn collect_names_from_pattern(pattern_node: Node, source_bytes: &[u8], declarations: &mut Vec<String>) {
    match pattern_node.kind() {
        "identifier" => {
            if let Ok(name) = pattern_node.utf8_text(source_bytes) {
                declarations.push(name.to_string());
            }
        }
        "object_pattern" | "array_pattern" => {
            let mut cursor = pattern_node.walk();
            for child in pattern_node.children(&mut cursor) {
                if child.kind() == "shorthand_property_identifier_pattern" || child.kind() == "identifier" {
                    if let Ok(name) = child.utf8_text(source_bytes) {
                        declarations.push(name.to_string());
                    }
                } else if child.kind() == "pair_pattern" {
                    if let Some(val) = child.child_by_field_name("value") {
                        collect_names_from_pattern(val, source_bytes, declarations);
                    }
                } else {
                    collect_names_from_pattern(child, source_bytes, declarations);
                }
            }
        }
        _ => {
            if let Ok(name) = pattern_node.utf8_text(source_bytes) {
                if !name.contains('{') && !name.contains('[') && !name.contains(' ') {
                    declarations.push(name.to_string());
                }
            }
        }
    }
}
