import { StateField, StateEffect, type Extension, type EditorState } from "@codemirror/state";
import { EditorView, Decoration, type DecorationSet, WidgetType, keymap } from "@codemirror/view";

export interface GhostTextPayload {
  text: string;
  pos: number;
}

export const setGhostTextEffect = StateEffect.define<GhostTextPayload | null>();

class GhostTextWidget extends WidgetType {
  constructor(readonly text: string) {
    super();
  }

  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-ghost-text";
    span.style.color = "#71717a";
    span.style.opacity = "0.85";
    span.style.fontStyle = "italic";
    span.style.pointerEvents = "none";
    span.style.userSelect = "none";
    span.style.fontFamily = "inherit";

    const textNode = document.createTextNode(this.text);
    span.appendChild(textNode);

    const badge = document.createElement("span");
    badge.className = "cm-ghost-text-badge";
    badge.textContent = " Tab ";
    badge.style.fontSize = "10px";
    badge.style.fontStyle = "normal";
    badge.style.backgroundColor = "rgba(113, 113, 122, 0.25)";
    badge.style.border = "1px solid #3f3f46";
    badge.style.borderRadius = "3px";
    badge.style.padding = "0 3px";
    badge.style.marginLeft = "4px";
    badge.style.color = "#a1a1aa";
    badge.style.verticalAlign = "middle";
    span.appendChild(badge);

    return span;
  }

  eq(other: GhostTextWidget): boolean {
    return other.text === this.text;
  }
}

export const ghostTextField = StateField.define<{
  decorations: DecorationSet;
  active: GhostTextPayload | null;
}>({
  create() {
    return { decorations: Decoration.none, active: null };
  },
  update(value, tr) {
    let nextActive = value.active;
    for (const effect of tr.effects) {
      if (effect.is(setGhostTextEffect)) {
        nextActive = effect.value;
      }
    }

    if (!nextActive || tr.docChanged) {
      if (tr.docChanged && nextActive) {
        // If doc changed, clear ghost text until next debounce cycle
        nextActive = null;
      }
    }

    if (!nextActive) {
      return { decorations: Decoration.none, active: null };
    }

    const deco = Decoration.widget({
      widget: new GhostTextWidget(nextActive.text),
      side: 1,
    }).range(nextActive.pos);

    return {
      decorations: Decoration.set([deco]),
      active: nextActive,
    };
  },
  provide: (field) => EditorView.decorations.from(field, (val) => val.decorations),
});

/**
 * Predicts smart predictive ghost-text completions from preceding code context.
 */
export function predictSmartCompletion(
  prefix: string,
  _suffix: string,
  filename: string,
): string | null {
  const trimmed = prefix.trimEnd();
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";

  // TypeScript / JavaScript predictions
  if (["ts", "tsx", "js", "jsx"].includes(ext)) {
    if (trimmed.endsWith("const [") || trimmed.endsWith("let [")) {
      return "state, setState] = useState(false);";
    }
    if (trimmed.endsWith("useEffect(() => {")) {
      return "\n    \n    return () => {};\n  }, []);";
    }
    if (trimmed.endsWith("useCallback((")) {
      return ") => {\n    \n  }, []);";
    }
    if (trimmed.endsWith("try {")) {
      return "\n    \n  } catch (error) {\n    console.error(error);\n  }";
    }
    if (trimmed.endsWith("import React from")) {
      return ' "react";';
    }
    if (trimmed.endsWith("console.log(")) {
      return '"[Debug]:", );';
    }
    if (trimmed.endsWith("async function") || trimmed.endsWith("async function ")) {
      return " execute() {\n  \n}";
    }
  }

  // Rust predictions
  if (ext === "rs") {
    if (trimmed.endsWith("pub async fn") || trimmed.endsWith("pub fn")) {
      return " handler() -> Result<(), String> {\n    Ok(())\n}";
    }
    if (trimmed.endsWith("impl")) {
      return " Default for MyStruct {\n    fn default() -> Self {\n        Self {}\n    }\n}";
    }
    if (trimmed.endsWith("match")) {
      return " res {\n        Ok(val) => val,\n        Err(err) => return Err(err.to_string()),\n    }";
    }
    if (trimmed.endsWith("println!(")) {
      return '"{:?}", );';
    }
  }

  // Python predictions
  if (ext === "py") {
    if (trimmed.endsWith("def __init__(self")) {
      return "):";
    }
    if (trimmed.endsWith("if __name__ ==")) {
      return ' "__main__":\n    main()';
    }
    if (trimmed.endsWith("with open(")) {
      return 'filepath, "r") as f:\n    content = f.read()';
    }
  }

  // Generic tag closure for HTML/JSX
  if (["html", "tsx", "jsx", "xml"].includes(ext)) {
    const tagMatch = trimmed.match(/<([a-zA-Z0-9_-]+)(?:\s+[^>]*)?>$/);
    if (tagMatch && tagMatch[1] && !["br", "hr", "img", "input"].includes(tagMatch[1])) {
      return `</${tagMatch[1]}>`;
    }
  }

  return null;
}

export function createGhostTextExtension(filename: string): Extension {
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const keymaps = keymap.of([
    {
      key: "Tab",
      run: (view: EditorView) => {
        const state = view.state.field(ghostTextField, false);
        if (state && state.active) {
          const { text, pos } = state.active;
          view.dispatch({
            changes: { from: pos, insert: text },
            selection: { anchor: pos + text.length },
            effects: setGhostTextEffect.of(null),
          });
          return true; // prevent tab indentation
        }
        return false;
      },
    },
    {
      key: "Escape",
      run: (view: EditorView) => {
        const state = view.state.field(ghostTextField, false);
        if (state && state.active) {
          view.dispatch({ effects: setGhostTextEffect.of(null) });
          return true;
        }
        return false;
      },
    },
  ]);

  const updateListener = EditorView.updateListener.of((update) => {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }

    if (update.docChanged || update.selectionSet) {
      const main = update.state.selection.main;
      if (!main.empty) {
        // Clear if selection range is non-empty
        const currentField = update.state.field(ghostTextField, false);
        if (currentField?.active) {
          update.view.dispatch({ effects: setGhostTextEffect.of(null) });
        }
        return;
      }

      const head = main.head;
      const line = update.state.doc.lineAt(head);
      const prefix = update.state.doc.sliceString(line.from, head);
      const suffix = update.state.doc.sliceString(head, line.to);

      debounceTimer = setTimeout(() => {
        const heuristic = predictSmartCompletion(prefix, suffix, filename);
        if (heuristic) {
          update.view.dispatch({
            effects: setGhostTextEffect.of({ text: heuristic, pos: head }),
          });
        }

        const contextPrefix = update.state.doc.sliceString(Math.max(0, head - 1600), head);
        if (contextPrefix.trim().length < 8) return;

        void import("@/utils/tauri-invoke")
          .then(({ invokeTauri }) =>
            invokeTauri<string>("run_inline_completion", {
              filename,
              prefix: contextPrefix,
            }),
          )
          .then((completion) => {
            if (!completion) return;
            const latestHead = update.view.state.selection.main.head;
            if (latestHead !== head) return;
            update.view.dispatch({
              effects: setGhostTextEffect.of({ text: completion, pos: head }),
            });
          })
          .catch(() => {
            // Keep heuristic suggestion when local LLM is offline
          });
      }, 280);
    }
  });

  return [ghostTextField, keymaps, updateListener];
}
