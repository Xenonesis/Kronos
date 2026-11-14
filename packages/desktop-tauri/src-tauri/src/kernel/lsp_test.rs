use std::path::Path;
use super::lsp::{LspHoverResult, LspMultiplexer, LspPosition, LspRange, LspServerConfig};

#[test]
fn test_lsp_detect_language_and_cmd() {
    let (lang_rs, cmd_rs, args_rs) =
        LspMultiplexer::detect_language_and_cmd(Path::new("src/main.rs")).expect("detect rs");
    assert_eq!(lang_rs, "rust");
    assert_eq!(cmd_rs, "rust-analyzer");
    assert!(args_rs.is_empty());

    let (lang_ts, cmd_ts, args_ts) =
        LspMultiplexer::detect_language_and_cmd(Path::new("packages/app/App.tsx")).expect("detect tsx");
    assert_eq!(lang_ts, "typescript");
    assert_eq!(cmd_ts, "typescript-language-server");
    assert_eq!(args_ts, &["--stdio"]);

    let (lang_py, cmd_py, args_py) =
        LspMultiplexer::detect_language_and_cmd(Path::new("scripts/test.py")).expect("detect py");
    assert_eq!(lang_py, "python");
    assert_eq!(cmd_py, "pyright-langserver");
    assert_eq!(args_py, &["--stdio"]);

    let (lang_go, cmd_go, _) =
        LspMultiplexer::detect_language_and_cmd(Path::new("backend/main.go")).expect("detect go");
    assert_eq!(lang_go, "go");
    assert_eq!(cmd_go, "gopls");

    assert!(LspMultiplexer::detect_language_and_cmd(Path::new("data/archive.bin")).is_none());
}

#[tokio::test]
async fn test_lsp_multiplexer_lifecycle() {
    let mux = LspMultiplexer::new();
    assert!(mux.list_servers().await.is_empty());

    // Stop non-existent server
    let stopped = mux.stop_server("non_existent").await.expect("stop server");
    assert!(!stopped);

    // Verify getting non-existent server is None
    assert!(mux.get_server("non_existent").await.is_none());
}

#[test]
fn test_lsp_models_serialization() {
    let hover = LspHoverResult {
        contents: "```rust\nfn main()\n```".to_string(),
        range: Some(LspRange {
            start: LspPosition { line: 0, character: 0 },
            end: LspPosition { line: 0, character: 7 },
        }),
    };

    let serialized = serde_json::to_string(&hover).expect("serialize hover");
    let deserialized: LspHoverResult = serde_json::from_str(&serialized).expect("deserialize hover");
    assert_eq!(hover, deserialized);

    let config = LspServerConfig {
        id: "rust-analyzer".to_string(),
        command: "rust-analyzer".to_string(),
        args: vec!["--log-file".to_string(), "ra.log".to_string()],
        root_path: ".".to_string(),
        initialization_options: None,
    };
    let config_json = serde_json::to_value(&config).expect("serialize config");
    assert_eq!(config_json["id"], "rust-analyzer");
    assert_eq!(config_json["args"][0], "--log-file");
}
