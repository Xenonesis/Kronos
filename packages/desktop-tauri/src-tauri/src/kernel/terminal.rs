use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSnapshot {
    pub rows: u16,
    pub cols: u16,
    pub cursor: (u16, u16),
    pub contents: String,
}

/// In-memory headless terminal screen powered by pure Rust vt100 emulator.
/// Tracks ANSI escape sequences, colors, scrollback, and cursor position without node-pty or @xterm/headless.
pub struct HeadlessTerminalScreen {
    parser: Mutex<vt100::Parser>,
    rows: u16,
    cols: u16,
}

impl HeadlessTerminalScreen {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: Mutex::new(vt100::Parser::new(rows, cols, 1000)),
            rows,
            cols,
        }
    }

    /// Feeds raw terminal output bytes through the ANSI state machine.
    pub fn process(&self, bytes: &[u8]) {
        if let Ok(mut parser) = self.parser.lock() {
            parser.process(bytes);
        }
    }

    /// Resizes the virtual terminal screen dimensions.
    pub fn resize(&self, rows: u16, cols: u16) {
        if let Ok(mut parser) = self.parser.lock() {
            parser.set_size(rows, cols);
        }
    }

    /// Extracts clean text representation of visible terminal contents.
    pub fn contents(&self) -> String {
        if let Ok(parser) = self.parser.lock() {
            parser.screen().contents()
        } else {
            String::new()
        }
    }

    /// Extracts current cursor position (row, col).
    pub fn cursor_position(&self) -> (u16, u16) {
        if let Ok(parser) = self.parser.lock() {
            let (r, c) = parser.screen().cursor_position();
            (r, c)
        } else {
            (0, 0)
        }
    }

    /// Generates a complete terminal snapshot for instant client reconnect.
    pub fn snapshot(&self) -> TerminalSnapshot {
        TerminalSnapshot {
            rows: self.rows,
            cols: self.cols,
            cursor: self.cursor_position(),
            contents: self.contents(),
        }
    }
}

/// Global terminal session registry managing headless screen tracking.
pub struct TerminalSessionRegistry {
    screens: RwLock<std::collections::HashMap<String, Arc<HeadlessTerminalScreen>>>,
}

impl TerminalSessionRegistry {
    pub fn new() -> Self {
        Self {
            screens: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub fn get_or_create(&self, id: &str, rows: u16, cols: u16) -> Arc<HeadlessTerminalScreen> {
        let mut map = self.screens.write().unwrap();
        map.entry(id.to_string())
            .or_insert_with(|| Arc::new(HeadlessTerminalScreen::new(rows, cols)))
            .clone()
    }

    pub fn remove(&self, id: &str) {
        let mut map = self.screens.write().unwrap();
        map.remove(id);
    }
}

static REGISTRY: std::sync::OnceLock<TerminalSessionRegistry> = std::sync::OnceLock::new();

pub fn get_terminal_registry() -> &'static TerminalSessionRegistry {
    REGISTRY.get_or_init(TerminalSessionRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_screen_ansi_parsing() {
        let screen = HeadlessTerminalScreen::new(24, 80);

        // Feed standard text
        screen.process(b"Hello Kronos Terminal\r\n");
        assert!(screen.contents().contains("Hello Kronos Terminal"));

        // Feed ANSI colored text and cursor movements
        screen.process(b"\x1b[32mGreen Text\x1b[0m\r\n");
        let contents = screen.contents();
        assert!(contents.contains("Green Text"));

        let (row, _col) = screen.cursor_position();
        assert!(row > 0);
    }

    #[test]
    fn test_screen_resize_and_snapshot() {
        let screen = HeadlessTerminalScreen::new(24, 80);
        screen.process(b"Status: Ready");

        let snap = screen.snapshot();
        assert_eq!(snap.rows, 24);
        assert_eq!(snap.cols, 80);
        assert!(snap.contents.contains("Status: Ready"));

        screen.resize(40, 120);
        let (_r, c) = screen.cursor_position();
        assert!(c < 120);
    }
}
