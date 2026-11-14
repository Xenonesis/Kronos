import { describe, expect, test, beforeEach } from "vitest";
import {
  useArtifactsStore,
  extractArtifactsFromSnippet,
} from "./use-artifacts-store";
import type { Artifact } from "./types";

describe("features/artifacts - useArtifactsStore", () => {
  beforeEach(() => {
    useArtifactsStore.getState().clearArtifacts();
  });

  test("initializes with empty artifacts and closed inspector", () => {
    const state = useArtifactsStore.getState();
    expect(state.artifacts).toEqual([]);
    expect(state.selectedArtifactId).toBeNull();
    expect(state.currentArtifact).toBeNull();
    expect(state.isInspectorOpen).toBe(false);
  });

  test("addArtifact prepends artifact, selects it, and updates currentArtifact", () => {
    const store = useArtifactsStore.getState();
    const created = store.addArtifact({
      title: "Test HTML Sandbox",
      type: "html",
      content: "<h1>Hello Kronos</h1>",
    });

    expect(created.id).toBeDefined();
    expect(created.title).toBe("Test HTML Sandbox");
    expect(created.type).toBe("html");

    const updated = useArtifactsStore.getState();
    expect(updated.artifacts.length).toBe(1);
    expect(updated.selectedArtifactId).toBe(created.id);
    expect(updated.currentArtifact?.content).toBe("<h1>Hello Kronos</h1>");
  });

  test("removeArtifact updates selectedArtifactId and currentArtifact gracefully", () => {
    const store = useArtifactsStore.getState();
    const a1 = store.addArtifact({
      id: "art-1",
      title: "First",
      type: "html",
      content: "<div id='1'></div>",
    });
    const a2 = store.addArtifact({
      id: "art-2",
      title: "Second",
      type: "svg",
      content: "<svg><circle r='10'/></svg>",
    });

    expect(useArtifactsStore.getState().selectedArtifactId).toBe("art-2");

    // Remove active artifact art-2
    useArtifactsStore.getState().removeArtifact("art-2");

    const afterRemove = useArtifactsStore.getState();
    expect(afterRemove.artifacts.length).toBe(1);
    expect(afterRemove.selectedArtifactId).toBe("art-1");
    expect(afterRemove.currentArtifact?.id).toBe("art-1");

    // Remove remaining artifact
    useArtifactsStore.getState().removeArtifact("art-1");
    const emptyState = useArtifactsStore.getState();
    expect(emptyState.artifacts.length).toBe(0);
    expect(emptyState.selectedArtifactId).toBeNull();
    expect(emptyState.currentArtifact).toBeNull();
  });

  test("selectArtifact switches active artifact", () => {
    const store = useArtifactsStore.getState();
    store.addArtifact({ id: "art-1", title: "First", type: "html", content: "one" });
    store.addArtifact({ id: "art-2", title: "Second", type: "svg", content: "two" });

    store.selectArtifact("art-1");
    expect(useArtifactsStore.getState().selectedArtifactId).toBe("art-1");
    expect(useArtifactsStore.getState().currentArtifact?.title).toBe("First");
  });

  test("openInspector and closeInspector toggle inspector visibility", () => {
    const store = useArtifactsStore.getState();
    const art = store.addArtifact({ id: "art-1", title: "First", type: "html", content: "one" });

    expect(useArtifactsStore.getState().isInspectorOpen).toBe(false);

    store.openInspector("art-1");
    expect(useArtifactsStore.getState().isInspectorOpen).toBe(true);
    expect(useArtifactsStore.getState().selectedArtifactId).toBe("art-1");

    store.closeInspector();
    expect(useArtifactsStore.getState().isInspectorOpen).toBe(false);
  });
});

describe("features/artifacts - extractArtifactsFromSnippet", () => {
  test("extracts HTML fences and titles", () => {
    const snippet = `
Here is a sample widget:
\`\`\`html
<!DOCTYPE html>
<html>
<head><title>Dashboard Preview</title></head>
<body><div class="card">Analytics</div></body>
</html>
\`\`\`
Hope this helps!
`;
    const artifacts = extractArtifactsFromSnippet(snippet, "msg-1");
    expect(artifacts.length).toBe(1);
    expect(artifacts[0].type).toBe("html");
    expect(artifacts[0].title).toBe("Dashboard Preview");
    expect(artifacts[0].content).toContain("Analytics");
  });

  test("extracts SVG fences", () => {
    const snippet = `
\`\`\`svg
<svg viewBox="0 0 100 100"><circle cx="50" cy="50" r="40" fill="purple" /></svg>
\`\`\`
`;
    const artifacts = extractArtifactsFromSnippet(snippet, "msg-svg");
    expect(artifacts.length).toBe(1);
    expect(artifacts[0].type).toBe("svg");
    expect(artifacts[0].title).toBe("SVG Graphic #1");
    expect(artifacts[0].content).toContain("<circle");
  });

  test("extracts React components", () => {
    const snippet = `
\`\`\`react
function App() {
  return <h1>Live React</h1>;
}
\`\`\`
`;
    const artifacts = extractArtifactsFromSnippet(snippet, "msg-react");
    expect(artifacts.length).toBe(1);
    expect(artifacts[0].type).toBe("react");
    expect(artifacts[0].title).toBe("React Component #1");
    expect(artifacts[0].content).toContain("Live React");
  });

  test("extracts standalone HTML document without markdown fences", () => {
    const doc = `<!DOCTYPE html><html><head><title>App Shell</title></head><body><h1>Paseo</h1></body></html>`;
    const artifacts = extractArtifactsFromSnippet(doc, "standalone");
    expect(artifacts.length).toBe(1);
    expect(artifacts[0].type).toBe("html");
    expect(artifacts[0].title).toBe("App Shell");
  });
});
