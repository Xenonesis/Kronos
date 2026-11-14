import React, { useState, useCallback, useMemo, useRef } from "react";
import {
  Monitor,
  Tablet,
  Smartphone,
  Sun,
  Moon,
  Grid2x2,
  RefreshCw,
  Copy,
  Check,
  ExternalLink,
  Code,
  Columns2,
  Eye,
} from "lucide-react-native";
import type {
  Artifact,
  ViewportMode,
  CanvasBackground,
  InspectMode,
  HtmlArtifactFrameProps,
} from "./types";

/**
 * Builds a self-contained HTML document for the iframe given artifact type and content.
 */
function buildIframeDocument(artifact: Artifact): string {
  const { type, content } = artifact;

  if (type === "svg") {
    return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      padding: 0;
      width: 100%;
      height: 100%;
      display: flex;
      align-items: center;
      justify-content: center;
      background: transparent;
      overflow: auto;
    }
    svg {
      max-width: 92%;
      max-height: 92%;
      height: auto;
      filter: drop-shadow(0 4px 12px rgba(0,0,0,0.15));
    }
  </style>
</head>
<body>
  ${content}
</body>
</html>`;
  }

  if (type === "markdown") {
    return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    body {
      margin: 0;
      padding: 24px;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      line-height: 1.6;
      color: #e4e4e7;
      background: #09090b;
    }
    pre {
      background: #18181b;
      padding: 12px;
      border-radius: 8px;
      overflow-x: auto;
      border: 1px solid #27272a;
    }
    code {
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      font-size: 0.9em;
    }
    h1, h2, h3, h4 { color: #fafafa; margin-top: 1.4em; }
    blockquote {
      border-left: 3px solid #a855f7;
      margin: 1em 0;
      padding-left: 14px;
      color: #a1a1aa;
    }
    table { border-collapse: collapse; width: 100%; margin: 1em 0; }
    th, td { border: 1px solid #27272a; padding: 8px 12px; text-align: left; }
    th { background: #18181b; }
  </style>
</head>
<body>
  <div style="max-width: 800px; margin: 0 auto; white-space: pre-wrap;">${escapeHtml(content)}</div>
</body>
</html>`;
  }

  if (type === "react") {
    return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    * { box-sizing: border-box; }
    body {
      margin: 0;
      padding: 16px;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: transparent;
    }
  </style>
  <script src="https://unpkg.com/react@18/umd/react.production.min.js"></script>
  <script src="https://unpkg.com/react-dom@18/umd/react-dom.production.min.js"></script>
  <script src="https://unpkg.com/@babel/standalone/babel.min.js"></script>
</head>
<body>
  <div id="root"></div>
  <script type="text/babel">
    try {
      ${content}
      if (typeof App !== 'undefined') {
        const root = ReactDOM.createRoot(document.getElementById('root'));
        root.render(<App />);
      }
    } catch (err) {
      document.getElementById('root').innerHTML = '<div style="color:#ef4444;font-family:monospace;padding:16px;">Runtime Error: ' + err.message + '</div>';
    }
  </script>
</body>
</html>`;
  }

  // Type === "html"
  if (/<html[\s>]/i.test(content) || /<!doctype html>/i.test(content)) {
    return content;
  }

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    *, *::before, *::after { box-sizing: border-box; }
    body {
      margin: 0;
      padding: 16px;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    }
  </style>
</head>
<body>
  ${content}
</body>
</html>`;
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export const HtmlArtifactFrame: React.FC<HtmlArtifactFrameProps> = ({
  artifact,
  defaultViewport = "desktop",
  defaultBackground = "dark",
  defaultInspectMode = "preview",
  onClose,
  style,
}) => {
  const [viewport, setViewport] = useState<ViewportMode>(defaultViewport);
  const [background, setBackground] = useState<CanvasBackground>(defaultBackground);
  const [inspectMode, setInspectMode] = useState<InspectMode>(defaultInspectMode);
  const [reloadKey, setReloadKey] = useState(0);
  const [copied, setCopied] = useState(false);
  const [isReloading, setIsReloading] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement | null>(null);

  const documentHtml = useMemo(() => buildIframeDocument(artifact), [artifact]);

  const handleRefresh = useCallback(() => {
    setIsReloading(true);
    setReloadKey((k) => k + 1);
    setTimeout(() => setIsReloading(false), 400);
  }, []);

  const handleCopyCode = useCallback(() => {
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(artifact.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [artifact.content]);

  const handleOpenInNewWindow = useCallback(async () => {
    const blob = new Blob([documentHtml], { type: "text/html;charset=utf-8" });
    const blobUrl = URL.createObjectURL(blob);

    if (typeof window !== "undefined" && window.paseoDesktop?.opener?.openUrl) {
      try {
        await window.paseoDesktop.opener.openUrl(blobUrl);
        return;
      } catch {
        // Fallback to window.open
      }
    }

    if (typeof window !== "undefined") {
      const win = window.open(blobUrl, "_blank");
      if (!win) {
        // Pop-up fallback
        const fallbackWin = window.open("", "_blank");
        if (fallbackWin) {
          fallbackWin.document.open();
          fallbackWin.document.write(documentHtml);
          fallbackWin.document.close();
        }
      }
    }
  }, [documentHtml]);

  // Canvas background styles
  const canvasBgStyle: React.CSSProperties = useMemo(() => {
    if (background === "checkerboard") {
      return {
        backgroundColor: "#18181b",
        backgroundImage:
          "linear-gradient(45deg, #27272a 25%, transparent 25%), linear-gradient(-45deg, #27272a 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #27272a 75%), linear-gradient(-45deg, transparent 75%, #27272a 75%)",
        backgroundSize: "20px 20px",
        backgroundPosition: "0 0, 0 10px, 10px -10px, -10px 0px",
      };
    }
    if (background === "light") {
      return { backgroundColor: "#ffffff" };
    }
    return { backgroundColor: "#09090b" };
  }, [background]);

  // Viewport container sizing
  const viewportContainerStyle: React.CSSProperties = useMemo(() => {
    if (viewport === "tablet") {
      return {
        width: "768px",
        maxWidth: "100%",
        height: "100%",
        maxHeight: "920px",
        margin: "0 auto",
        borderRadius: "16px",
        border: "8px solid #27272a",
        boxShadow: "0 25px 60px -15px rgba(0, 0, 0, 0.7)",
        position: "relative",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        backgroundColor: background === "light" ? "#ffffff" : "#09090b",
      };
    }
    if (viewport === "mobile") {
      return {
        width: "375px",
        maxWidth: "100%",
        height: "100%",
        maxHeight: "780px",
        margin: "0 auto",
        borderRadius: "24px",
        border: "10px solid #27272a",
        boxShadow: "0 25px 60px -15px rgba(0, 0, 0, 0.8)",
        position: "relative",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        backgroundColor: background === "light" ? "#ffffff" : "#09090b",
      };
    }
    return {
      width: "100%",
      height: "100%",
      position: "relative",
      display: "flex",
      flexDirection: "column",
      backgroundColor: background === "light" ? "#ffffff" : "transparent",
    };
  }, [viewport, background]);

  return (
    <div
      data-testid="html-artifact-frame"
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        backgroundColor: "#121214",
        color: "#f4f4f5",
        fontFamily: "system-ui, -apple-system, sans-serif",
        overflow: "hidden",
        ...style,
      }}
    >
      {/* Controls Toolbar */}
      <div
        data-testid="artifact-toolbar"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "8px 14px",
          backgroundColor: "#18181b",
          borderBottom: "1px solid #27272a",
          fontSize: "12px",
          gap: "12px",
          flexWrap: "wrap",
          zIndex: 10,
        }}
      >
        {/* Left Section: Viewport Switcher */}
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          <span style={{ color: "#71717a", marginRight: "2px", fontSize: "11px", fontWeight: 500 }}>
            VIEWPORT:
          </span>
          <div
            style={{
              display: "flex",
              backgroundColor: "#27272a",
              padding: "2px",
              borderRadius: "6px",
              gap: "2px",
            }}
          >
            <button
              data-testid="viewport-desktop"
              onClick={() => setViewport("desktop")}
              title="Desktop View (100%)"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "4px",
                padding: "4px 8px",
                borderRadius: "4px",
                border: "none",
                cursor: "pointer",
                backgroundColor: viewport === "desktop" ? "#3f3f46" : "transparent",
                color: viewport === "desktop" ? "#ffffff" : "#a1a1aa",
                fontSize: "11px",
              }}
            >
              <Monitor size={13} />
              <span>Desktop</span>
            </button>
            <button
              data-testid="viewport-tablet"
              onClick={() => setViewport("tablet")}
              title="Tablet View (768px)"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "4px",
                padding: "4px 8px",
                borderRadius: "4px",
                border: "none",
                cursor: "pointer",
                backgroundColor: viewport === "tablet" ? "#3f3f46" : "transparent",
                color: viewport === "tablet" ? "#ffffff" : "#a1a1aa",
                fontSize: "11px",
              }}
            >
              <Tablet size={13} />
              <span>Tablet</span>
            </button>
            <button
              data-testid="viewport-mobile"
              onClick={() => setViewport("mobile")}
              title="Mobile View (375px)"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "4px",
                padding: "4px 8px",
                borderRadius: "4px",
                border: "none",
                cursor: "pointer",
                backgroundColor: viewport === "mobile" ? "#3f3f46" : "transparent",
                color: viewport === "mobile" ? "#ffffff" : "#a1a1aa",
                fontSize: "11px",
              }}
            >
              <Smartphone size={13} />
              <span>Mobile</span>
            </button>
          </div>
        </div>

        {/* Center Section: Canvas Background & View Mode */}
        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          {/* Background Toggle */}
          <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
            <span style={{ color: "#71717a", fontSize: "11px", fontWeight: 500 }}>CANVAS:</span>
            <div
              style={{
                display: "flex",
                backgroundColor: "#27272a",
                padding: "2px",
                borderRadius: "6px",
                gap: "2px",
              }}
            >
              <button
                data-testid="bg-dark"
                onClick={() => setBackground("dark")}
                title="Solid Dark"
                style={{
                  display: "flex",
                  alignItems: "center",
                  padding: "4px 6px",
                  borderRadius: "4px",
                  border: "none",
                  cursor: "pointer",
                  backgroundColor: background === "dark" ? "#3f3f46" : "transparent",
                  color: background === "dark" ? "#ffffff" : "#a1a1aa",
                }}
              >
                <Moon size={13} />
              </button>
              <button
                data-testid="bg-checkerboard"
                onClick={() => setBackground("checkerboard")}
                title="Checkerboard Pattern (Transparency)"
                style={{
                  display: "flex",
                  alignItems: "center",
                  padding: "4px 6px",
                  borderRadius: "4px",
                  border: "none",
                  cursor: "pointer",
                  backgroundColor: background === "checkerboard" ? "#3f3f46" : "transparent",
                  color: background === "checkerboard" ? "#ffffff" : "#a1a1aa",
                }}
              >
                <Grid2x2 size={13} />
              </button>
              <button
                data-testid="bg-light"
                onClick={() => setBackground("light")}
                title="Solid Light"
                style={{
                  display: "flex",
                  alignItems: "center",
                  padding: "4px 6px",
                  borderRadius: "4px",
                  border: "none",
                  cursor: "pointer",
                  backgroundColor: background === "light" ? "#3f3f46" : "transparent",
                  color: background === "light" ? "#ffffff" : "#a1a1aa",
                }}
              >
                <Sun size={13} />
              </button>
            </div>
          </div>

          {/* Inspect Code Mode Toggle */}
          <div
            style={{
              display: "flex",
              backgroundColor: "#27272a",
              padding: "2px",
              borderRadius: "6px",
              gap: "2px",
            }}
          >
            <button
              data-testid="mode-preview"
              onClick={() => setInspectMode("preview")}
              title="Preview Only"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "4px",
                padding: "4px 8px",
                borderRadius: "4px",
                border: "none",
                cursor: "pointer",
                backgroundColor: inspectMode === "preview" ? "#3f3f46" : "transparent",
                color: inspectMode === "preview" ? "#c084fc" : "#a1a1aa",
                fontSize: "11px",
              }}
            >
              <Eye size={13} />
              <span>Preview</span>
            </button>
            <button
              data-testid="mode-split"
              onClick={() => setInspectMode("split")}
              title="Split View (Preview & Raw Code)"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "4px",
                padding: "4px 8px",
                borderRadius: "4px",
                border: "none",
                cursor: "pointer",
                backgroundColor: inspectMode === "split" ? "#3f3f46" : "transparent",
                color: inspectMode === "split" ? "#c084fc" : "#a1a1aa",
                fontSize: "11px",
              }}
            >
              <Columns2 size={13} />
              <span>Split</span>
            </button>
            <button
              data-testid="mode-code"
              onClick={() => setInspectMode("code")}
              title="Inspect Raw Code"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "4px",
                padding: "4px 8px",
                borderRadius: "4px",
                border: "none",
                cursor: "pointer",
                backgroundColor: inspectMode === "code" ? "#3f3f46" : "transparent",
                color: inspectMode === "code" ? "#c084fc" : "#a1a1aa",
                fontSize: "11px",
              }}
            >
              <Code size={13} />
              <span>Inspect Code</span>
            </button>
          </div>
        </div>

        {/* Right Section: Action Buttons */}
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          {/* Refresh / Reload Button */}
          <button
            data-testid="artifact-refresh-btn"
            onClick={handleRefresh}
            title="Reload Iframe Sandbox"
            style={{
              display: "flex",
              alignItems: "center",
              gap: "4px",
              padding: "5px 9px",
              backgroundColor: "#27272a",
              border: "1px solid #3f3f46",
              borderRadius: "6px",
              color: "#e4e4e7",
              fontSize: "11px",
              cursor: "pointer",
            }}
          >
            <span
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                transform: isReloading ? "rotate(180deg)" : "rotate(0deg)",
                transition: "transform 0.4s ease",
              }}
            >
              <RefreshCw size={12} />
            </span>
            <span>Reload</span>
          </button>

          {/* Copy Code Button */}
          <button
            data-testid="artifact-copy-btn"
            onClick={handleCopyCode}
            title="Copy Raw Code"
            style={{
              display: "flex",
              alignItems: "center",
              gap: "4px",
              padding: "5px 9px",
              backgroundColor: copied ? "rgba(34, 197, 94, 0.2)" : "#27272a",
              border: copied ? "1px solid #22c55e" : "1px solid #3f3f46",
              borderRadius: "6px",
              color: copied ? "#4ade80" : "#e4e4e7",
              fontSize: "11px",
              cursor: "pointer",
            }}
          >
            {copied ? <Check size={12} color="#22c55e" /> : <Copy size={12} />}
            <span>{copied ? "Copied!" : "Copy"}</span>
          </button>

          {/* Open in New Window */}
          <button
            data-testid="artifact-open-window-btn"
            onClick={handleOpenInNewWindow}
            title="Open in New Window / Browser"
            style={{
              display: "flex",
              alignItems: "center",
              gap: "4px",
              padding: "5px 9px",
              backgroundColor: "#27272a",
              border: "1px solid #3f3f46",
              borderRadius: "6px",
              color: "#e4e4e7",
              fontSize: "11px",
              cursor: "pointer",
            }}
          >
            <ExternalLink size={12} />
            <span>Open Browser</span>
          </button>
        </div>
      </div>

      {/* Main Content Area (Split / Preview / Code) */}
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "row",
          overflow: "hidden",
          position: "relative",
        }}
      >
        {/* Preview Frame Container */}
        {(inspectMode === "preview" || inspectMode === "split") && (
          <div
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              padding: viewport === "desktop" ? "0px" : "20px",
              overflow: "auto",
              position: "relative",
              ...canvasBgStyle,
            }}
          >
            <div style={viewportContainerStyle}>
              {/* Optional Phone/Tablet Top Camera Notch */}
              {viewport === "mobile" && (
                <div
                  style={{
                    position: "absolute",
                    top: 4,
                    left: "50%",
                    transform: "translateX(-50%)",
                    width: "72px",
                    height: "12px",
                    backgroundColor: "#27272a",
                    borderRadius: "6px",
                    zIndex: 20,
                  }}
                />
              )}

              {/* Sandboxed iframe */}
              <iframe
                key={reloadKey}
                ref={iframeRef}
                srcDoc={documentHtml}
                title={artifact.title}
                sandbox="allow-scripts allow-modals allow-forms"
                style={{
                  width: "100%",
                  height: "100%",
                  border: "none",
                  backgroundColor: "transparent",
                  flex: 1,
                }}
              />
            </div>
          </div>
        )}

        {/* Code Inspection Panel */}
        {(inspectMode === "code" || inspectMode === "split") && (
          <div
            data-testid="artifact-code-inspector"
            style={{
              width: inspectMode === "split" ? "45%" : "100%",
              maxWidth: inspectMode === "split" ? "50%" : "100%",
              borderLeft: inspectMode === "split" ? "1px solid #27272a" : "none",
              backgroundColor: "#0d0d0f",
              display: "flex",
              flexDirection: "column",
              overflow: "hidden",
            }}
          >
            {/* Code Header Bar */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "8px 14px",
                borderBottom: "1px solid #27272a",
                backgroundColor: "#141416",
                fontSize: "11px",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <Code size={13} color="#a855f7" />
                <span style={{ fontWeight: 600, color: "#d4d4d8" }}>
                  Source: {artifact.title}
                </span>
                <span
                  style={{
                    backgroundColor: "rgba(168, 85, 247, 0.15)",
                    color: "#c084fc",
                    padding: "1px 6px",
                    borderRadius: "4px",
                    fontSize: "10px",
                    textTransform: "uppercase",
                    fontWeight: 600,
                  }}
                >
                  {artifact.type}
                </span>
              </div>
              <span style={{ color: "#71717a" }}>{artifact.content.length} bytes</span>
            </div>

            {/* Code Body */}
            <div
              style={{
                flex: 1,
                overflow: "auto",
                padding: "14px",
                fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
                fontSize: "12px",
                lineHeight: "1.6",
                color: "#e4e4e7",
                backgroundColor: "#0d0d0f",
                whiteSpace: "pre",
              }}
            >
              {artifact.content}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
