/**
 * @vitest-environment jsdom
 */
import React from "react";
import { cleanup, render, screen, fireEvent } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("lucide-react-native", () => {
  const createIcon = (name: string) => (props: Record<string, unknown>) =>
    React.createElement("span", { ...props, "data-icon": name });
  return {
    Monitor: createIcon("Monitor"),
    Tablet: createIcon("Tablet"),
    Smartphone: createIcon("Smartphone"),
    Sun: createIcon("Sun"),
    Moon: createIcon("Moon"),
    Grid2x2: createIcon("Grid2x2"),
    RefreshCw: createIcon("RefreshCw"),
    Copy: createIcon("Copy"),
    Check: createIcon("Check"),
    ExternalLink: createIcon("ExternalLink"),
    Code: createIcon("Code"),
    Columns2: createIcon("Columns2"),
    Eye: createIcon("Eye"),
    X: createIcon("X"),
    Maximize2: createIcon("Maximize2"),
    Minimize2: createIcon("Minimize2"),
    Sparkles: createIcon("Sparkles"),
    Clock: createIcon("Clock"),
    Layers: createIcon("Layers"),
    FileText: createIcon("FileText"),
  };
});

import { HtmlArtifactFrame } from "./HtmlArtifactFrame";
import { ArtifactSnippetChip } from "./ArtifactSnippetChip";
import { ArtifactInspectorModal } from "./ArtifactInspectorModal";
import { useArtifactsStore } from "./use-artifacts-store";
import type { Artifact } from "./types";

afterEach(() => {
  cleanup();
  useArtifactsStore.getState().clearArtifacts();
});

const mockArtifact: Artifact = {
  id: "test-art-1",
  title: "Interactive Calculator",
  type: "html",
  content: "<div id='calc'><h1>Calculator</h1><button id='btn'>1</button></div>",
  createdAt: Date.now(),
};

describe("HtmlArtifactFrame Component", () => {
  it("renders sandboxed iframe with allow-scripts allow-modals allow-forms", () => {
    const { container } = render(<HtmlArtifactFrame artifact={mockArtifact} />);
    const iframe = container.querySelector("iframe");
    expect(iframe).toBeTruthy();
    expect(iframe?.getAttribute("sandbox")).toBe("allow-scripts allow-modals allow-forms");
    expect(iframe?.getAttribute("srcdoc")).toContain("Calculator");
  });

  it("renders controls toolbar with viewport and background switchers", () => {
    render(<HtmlArtifactFrame artifact={mockArtifact} />);
    expect(screen.getByTestId("artifact-toolbar")).toBeTruthy();
    expect(screen.getByTestId("viewport-desktop")).toBeTruthy();
    expect(screen.getByTestId("viewport-tablet")).toBeTruthy();
    expect(screen.getByTestId("viewport-mobile")).toBeTruthy();
    expect(screen.getByTestId("bg-dark")).toBeTruthy();
    expect(screen.getByTestId("bg-checkerboard")).toBeTruthy();
    expect(screen.getByTestId("bg-light")).toBeTruthy();
  });

  it("switches viewport when clicking tablet and mobile buttons", () => {
    render(<HtmlArtifactFrame artifact={mockArtifact} />);
    const tabletBtn = screen.getByTestId("viewport-tablet");
    fireEvent.click(tabletBtn);

    const mobileBtn = screen.getByTestId("viewport-mobile");
    fireEvent.click(mobileBtn);
  });

  it("switches background when clicking background buttons", () => {
    render(<HtmlArtifactFrame artifact={mockArtifact} />);
    const checkerBtn = screen.getByTestId("bg-checkerboard");
    fireEvent.click(checkerBtn);

    const lightBtn = screen.getByTestId("bg-light");
    fireEvent.click(lightBtn);
  });

  it("switches view modes to code and split", () => {
    render(<HtmlArtifactFrame artifact={mockArtifact} />);
    expect(screen.queryByTestId("artifact-code-inspector")).toBeNull();

    // Click split mode
    const splitBtn = screen.getByTestId("mode-split");
    fireEvent.click(splitBtn);
    expect(screen.getByTestId("artifact-code-inspector")).toBeTruthy();
    expect(screen.getByText(/Source: Interactive Calculator/)).toBeTruthy();

    // Click code mode
    const codeBtn = screen.getByTestId("mode-code");
    fireEvent.click(codeBtn);
    expect(screen.getByTestId("artifact-code-inspector")).toBeTruthy();

    // Return to preview
    const previewBtn = screen.getByTestId("mode-preview");
    fireEvent.click(previewBtn);
    expect(screen.queryByTestId("artifact-code-inspector")).toBeNull();
  });

  it("handles copy code button and triggers clipboard write", () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextMock,
      },
    });

    render(<HtmlArtifactFrame artifact={mockArtifact} />);
    const copyBtn = screen.getByTestId("artifact-copy-btn");
    fireEvent.click(copyBtn);
    expect(writeTextMock).toHaveBeenCalledWith(mockArtifact.content);
  });

  it("handles reload button", () => {
    render(<HtmlArtifactFrame artifact={mockArtifact} />);
    const reloadBtn = screen.getByTestId("artifact-refresh-btn");
    fireEvent.click(reloadBtn);
  });
});

describe("ArtifactSnippetChip Component", () => {
  it("renders sleek preview badge with title and type", () => {
    render(
      <ArtifactSnippetChip
        title="Vector Logo"
        type="svg"
        content="<svg></svg>"
      />,
    );
    expect(screen.getByTestId("artifact-snippet-chip")).toBeTruthy();
    expect(screen.getByText(/Vector Logo/)).toBeTruthy();
    expect(screen.getByText(/svg/i)).toBeTruthy();
  });

  it("opens inspector store on click", () => {
    render(
      <ArtifactSnippetChip
        title="Vector Logo"
        type="svg"
        content="<svg></svg>"
      />,
    );
    const chip = screen.getByTestId("artifact-snippet-chip");
    fireEvent.click(chip);

    const state = useArtifactsStore.getState();
    expect(state.isInspectorOpen).toBe(true);
    expect(state.currentArtifact?.title).toBe("Vector Logo");
  });
});

describe("ArtifactInspectorModal Component", () => {
  it("does not render when visible is false and store is closed", () => {
    const { container } = render(<ArtifactInspectorModal visible={false} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders modal when visible is true with header and close action", () => {
    const onCloseMock = vi.fn();
    render(
      <ArtifactInspectorModal
        visible={true}
        artifact={mockArtifact}
        onClose={onCloseMock}
      />,
    );

    expect(screen.getByTestId("artifact-inspector-modal")).toBeTruthy();
    expect(screen.getByText("Interactive Calculator")).toBeTruthy();

    const closeBtn = screen.getByTestId("artifact-close-modal-btn");
    fireEvent.click(closeBtn);
    expect(onCloseMock).toHaveBeenCalled();
  });

  it("toggles fullscreen mode", () => {
    render(
      <ArtifactInspectorModal
        visible={true}
        artifact={mockArtifact}
      />,
    );
    const fullscreenBtn = screen.getByTestId("artifact-fullscreen-btn");
    fireEvent.click(fullscreenBtn);
    fireEvent.click(fullscreenBtn);
  });
});
