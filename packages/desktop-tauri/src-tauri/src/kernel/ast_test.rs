#[cfg(test)]
mod tests {
    use crate::kernel::ast::{AstEngine, AstSymbolKind};

    #[test]
    fn test_ast_rich_symbol_extraction() {
        let engine = AstEngine::new();

        let source = r#"
            /**
             * Authenticates a user with a JWT token.
             */
            export async function authenticate(token: string): Promise<boolean> {
                return token.length > 0;
            }

            export const calculateSum = (a: number, b: number): number => a + b;

            export interface ConfigOptions {
                port: number;
                host?: string;
                isValid(): boolean;
            }

            export class WebServer {
                private running: boolean = false;

                constructor(private options: ConfigOptions) {}

                /** Starts the server */
                public async start(): Promise<void> {
                    this.running = true;
                }
            }

            export enum ServerStatus {
                Stopped,
                Starting,
                Running,
            }

            export type PortNumber = number;
        "#;

        let result = engine
            .analyze(source, "ts")
            .expect("Analysis should succeed");

        assert_eq!(result.root_kind, "program");
        assert!(result.errors.is_empty(), "Should have no syntax errors");

        // Verify symbols
        let auth_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "authenticate")
            .expect("Should find authenticate symbol");
        assert_eq!(auth_sym.kind, AstSymbolKind::AsyncFunction);
        assert!(auth_sym.exported);
        assert!(auth_sym.doc_comment.is_some());
        assert!(auth_sym.doc_comment.as_ref().unwrap().contains("Authenticates a user"));

        // Arrow function
        let sum_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "calculateSum")
            .expect("Should find calculateSum symbol");
        assert_eq!(sum_sym.kind, AstSymbolKind::ArrowFunction);
        assert!(sum_sym.exported);

        // Interface with members
        let iface_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "ConfigOptions")
            .expect("Should find ConfigOptions symbol");
        assert_eq!(iface_sym.kind, AstSymbolKind::Interface);
        assert!(iface_sym.exported);
        assert!(iface_sym.children.iter().any(|c| c.name == "port" && c.kind == AstSymbolKind::Property));
        assert!(iface_sym.children.iter().any(|c| c.name == "isValid" && c.kind == AstSymbolKind::Method));

        // Class with constructor and methods
        let class_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "WebServer")
            .expect("Should find WebServer symbol");
        assert_eq!(class_sym.kind, AstSymbolKind::Class);
        assert!(class_sym.exported);
        assert!(class_sym.children.iter().any(|c| c.name == "constructor" && c.kind == AstSymbolKind::Constructor));
        let start_method = class_sym.children.iter().find(|c| c.name == "start").expect("start method");
        assert_eq!(start_method.kind, AstSymbolKind::Method);
        assert!(start_method.doc_comment.as_ref().map(|d| d.contains("Starts the server")).unwrap_or(false));

        // Enum with members
        let enum_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "ServerStatus")
            .expect("Should find ServerStatus symbol");
        assert_eq!(enum_sym.kind, AstSymbolKind::Enum);
        assert_eq!(enum_sym.children.len(), 3);
        assert_eq!(enum_sym.children[0].name, "Stopped");
        assert_eq!(enum_sym.children[1].name, "Starting");
        assert_eq!(enum_sym.children[2].name, "Running");

        // Type alias
        let type_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "PortNumber")
            .expect("Should find PortNumber symbol");
        assert_eq!(type_sym.kind, AstSymbolKind::TypeAlias);
    }

    #[test]
    fn test_ast_imports_extraction() {
        let engine = AstEngine::new();

        let source = r#"
            import React, { useState, useEffect } from 'react';
            import type { AppProps } from './types';
            import * as fs from 'node:fs';
        "#;

        let imports = engine
            .extract_imports(source, "tsx")
            .expect("Import extraction should succeed");

        assert_eq!(imports.len(), 3);

        // React import
        let react_imp = imports.iter().find(|i| i.source == "react").expect("react import");
        assert!(react_imp.is_default);
        assert!(react_imp.specifiers.contains(&"React".to_string()));
        assert!(react_imp.specifiers.contains(&"useState".to_string()));
        assert!(react_imp.specifiers.contains(&"useEffect".to_string()));
        assert!(!react_imp.is_type_only);

        // Type only import
        let types_imp = imports.iter().find(|i| i.source == "./types").expect("types import");
        assert!(types_imp.is_type_only);
        assert!(types_imp.specifiers.contains(&"AppProps".to_string()));

        // Namespace import
        let fs_imp = imports.iter().find(|i| i.source == "node:fs").expect("fs import");
        assert!(fs_imp.specifiers.iter().any(|s| s.contains("fs")));
    }

    #[test]
    fn test_ast_find_symbol_at_position() {
        let engine = AstEngine::new();

        let source = r#"
export class EditorService {
    public getBuffer(): string {
        return "buffer-content";
    }
}
"#;

        // Line 3 is inside getBuffer method
        let sym = engine
            .find_symbol_at_position(source, "ts", 3, 10)
            .expect("Position lookup should succeed")
            .expect("Symbol should be found");

        assert_eq!(sym.name, "getBuffer");
        assert_eq!(sym.kind, AstSymbolKind::Method);

        // Line 2 is on EditorService class declaration
        let class_sym = engine
            .find_symbol_at_position(source, "ts", 2, 5)
            .expect("Position lookup should succeed")
            .expect("Class symbol should be found");

        assert_eq!(class_sym.name, "EditorService");
        assert_eq!(class_sym.kind, AstSymbolKind::Class);
    }

    #[test]
    fn test_ast_syntax_error_detection() {
        let engine = AstEngine::new();

        let invalid_source = r#"
            function broken( {
                return 42;
        "#;

        let errors = engine
            .detect_errors(invalid_source, "ts")
            .expect("Detection should run");

        assert!(!errors.is_empty(), "Should detect syntax error in malformed code");
        assert!(errors[0].message.contains("Syntax error") || errors[0].message.contains("Missing"));
    }

    #[test]
    fn test_ast_parsing_performance_benchmark() {
        use std::time::Instant;

        let engine = AstEngine::new();

        // Generate a synthetic 2,500-line TypeScript file with functions, classes, and interfaces
        let mut source = String::with_capacity(100_000);
        source.push_str("import { EventEmitter } from 'events';\n\n");

        for i in 0..50 {
            source.push_str(&format!(
                r#"
                export interface ConfigPayload_{i} {{
                    id: number;
                    name: string;
                    active: boolean;
                }}

                export class Controller_{i} {{
                    private count = {i};
                    public handle(data: ConfigPayload_{i}): boolean {{
                        return data.active && this.count > 0;
                    }}
                }}

                export async function processTask_{i}(payload: ConfigPayload_{i}): Promise<string> {{
                    return `result-${{payload.id}}`;
                }}
                "#
            ));
        }

        let start = Instant::now();
        let result = engine
            .analyze(&source, "ts")
            .expect("High-volume analysis should succeed");
        let elapsed = start.elapsed();

        println!(
            "[AST Benchmark] Parsed {} nodes, {} symbols, {} declarations in {:?}",
            result.node_count,
            result.symbols.len(),
            result.declarations.len(),
            elapsed
        );

        assert!(result.node_count > 1000, "Should have parsed over 1000 nodes");
        assert_eq!(result.symbols.len(), 150, "Should have 150 top-level symbols (50 ifaces + 50 classes + 50 functions)");
        assert!(result.errors.is_empty(), "Should have 0 syntax errors");
        assert!(elapsed.as_millis() < 50, "Expected sub-50ms parse time for 1,500+ LOC TypeScript file, took {:?}", elapsed);
    }
}
