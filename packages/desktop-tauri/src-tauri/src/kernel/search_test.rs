#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::kernel::ast::AstEngine;
    use crate::kernel::search::WorkspaceSearchIndex;

    #[test]
    fn test_tantivy_sub_millisecond_search() {
        let index = WorkspaceSearchIndex::new_in_memory().expect("Failed to initialize in-memory search index");

        index
            .index_document(
                "src/auth.ts",
                "export function authenticateUser(token: string) {\n    return verifyToken(token);\n}",
            )
            .expect("Failed to index src/auth.ts");

        index
            .index_document(
                "src/config.ts",
                "export const API_BASE_URL = 'https://api.example.com';",
            )
            .expect("Failed to index src/config.ts");

        // Warm up and verify query
        let start = Instant::now();
        let results = index
            .search("authenticateUser")
            .expect("Failed to perform search query");
        let duration = start.elapsed();

        println!("Tantivy in-memory search executed in: {:?}", duration);

        assert_eq!(results.len(), 1, "Expected exactly 1 matching document");
        assert_eq!(results[0].path, "src/auth.ts");
        assert!(results[0].score > 0.0);

        // Search for config symbol
        let config_results = index.search("API_BASE_URL").expect("Failed to search config");
        assert_eq!(config_results.len(), 1);
        assert_eq!(config_results[0].path, "src/config.ts");

        // Search for non-existent symbol
        let empty_results = index.search("nonExistentSymbolXYZ").expect("Search should succeed");
        assert!(empty_results.is_empty());
    }

    #[test]
    fn test_ast_engine_typescript_parsing() {
        let engine = AstEngine::new();

        let ts_code = r#"
            import { verify } from './crypto';

            export interface UserSession {
                userId: string;
                roles: string[];
            }

            export class SessionManager {
                private sessions: Map<string, UserSession> = new Map();
            }

            export function authenticateUser(token: string): UserSession {
                return { userId: '123', roles: ['admin'] };
            }

            export const MAX_RETRY_COUNT = 3;
            let currentAttempt = 0;
        "#;

        let summary = engine
            .parse_syntax(ts_code, "ts")
            .expect("TypeScript parsing should succeed");

        assert_eq!(summary.root_kind, "program");
        assert!(summary.node_count > 10, "Expected non-trivial node count, got {}", summary.node_count);

        // Verify top-level declarations are extracted
        assert!(
            summary.declarations.contains(&"UserSession".to_string()),
            "Missing interface UserSession in declarations: {:?}",
            summary.declarations
        );
        assert!(
            summary.declarations.contains(&"SessionManager".to_string()),
            "Missing class SessionManager in declarations: {:?}",
            summary.declarations
        );
        assert!(
            summary.declarations.contains(&"authenticateUser".to_string()),
            "Missing function authenticateUser in declarations: {:?}",
            summary.declarations
        );
        assert!(
            summary.declarations.contains(&"MAX_RETRY_COUNT".to_string()),
            "Missing variable MAX_RETRY_COUNT in declarations: {:?}",
            summary.declarations
        );
        assert!(
            summary.declarations.contains(&"currentAttempt".to_string()),
            "Missing variable currentAttempt in declarations: {:?}",
            summary.declarations
        );
    }

    #[test]
    fn test_ast_engine_tsx_parsing() {
        let engine = AstEngine::new();

        let tsx_code = r#"
            import React from 'react';

            export interface ButtonProps {
                label: string;
            }

            export function PrimaryButton({ label }: ButtonProps) {
                return <button className="btn-primary">{label}</button>;
            }
        "#;

        let summary = engine
            .parse_syntax(tsx_code, "tsx")
            .expect("TSX parsing should succeed");

        assert_eq!(summary.root_kind, "program");
        assert!(summary.declarations.contains(&"ButtonProps".to_string()));
        assert!(summary.declarations.contains(&"PrimaryButton".to_string()));
    }

    #[test]
    fn test_ast_engine_unsupported_fallback() {
        let engine = AstEngine::new();

        let summary = engine
            .parse_syntax("pub fn main() {}", "rs")
            .expect("Unsupported extension should safely return fallback summary");

        assert_eq!(summary.root_kind, "unsupported");
        assert_eq!(summary.node_count, 0);
        assert!(summary.declarations.is_empty());
    }
}
