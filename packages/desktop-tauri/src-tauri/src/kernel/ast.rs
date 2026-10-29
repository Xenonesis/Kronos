use tree_sitter::{Node, Parser};

/// Summary of an Abstract Syntax Tree (AST) parsed by Tree-Sitter.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AstSummary {
    pub root_kind: String,
    pub node_count: usize,
    pub declarations: Vec<String>,
}

/// Native Tree-Sitter AST Engine for fast syntax inspection and declaration extraction.
#[derive(Debug, Clone, Default)]
pub struct AstEngine;

impl AstEngine {
    /// Creates a new `AstEngine` instance.
    pub fn new() -> Self {
        Self
    }

    /// Parses the syntax of the provided source code based on file extension.
    /// Supports TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`, `.mts`, `.cts`, `.mjs`, `.cjs`).
    /// Provides a safe fallback for unsupported extensions.
    pub fn parse_syntax(&self, source: &str, ext: &str) -> Result<AstSummary, String> {
        let clean_ext = ext.trim_start_matches('.').to_lowercase();
        let language = match clean_ext.as_str() {
            "ts" | "mts" | "cts" | "js" | "mjs" | "cjs" => {
                tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)
            }
            "tsx" | "jsx" => {
                tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TSX)
            }
            _ => {
                // Safe fallback for unsupported extensions
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
            // Check for inner declaration inside export statement
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
