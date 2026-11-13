import React, { memo, useState, useCallback, useEffect } from "react";
import {
  Cloud,
  Rocket,
  CheckCircle2,
  ExternalLink,
  Copy,
  X,
  Server,
  Terminal,
  Loader2,
  Check,
} from "lucide-react-native";
import { useToast } from "@/contexts/toast-context";
import { invokeTauri } from "@/utils/tauri-invoke";

export type CloudProviderId = "vercel" | "netlify" | "cloudflare";

export interface CloudDeploymentModalProps {
  visible: boolean;
  cwd: string;
  projectName?: string;
  onClose: () => void;
}

export const CloudDeploymentModal = memo(function CloudDeploymentModal({
  visible,
  cwd,
  projectName = "kronos-web-app",
  onClose,
}: CloudDeploymentModalProps) {
  const [provider, setProvider] = useState<CloudProviderId>("vercel");
  const [name, setName] = useState(projectName);
  const [outputDir, setOutputDir] = useState("dist");
  const [isDeploying, setIsDeploying] = useState(false);
  const [deployedUrl, setDeployedUrl] = useState<string | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [copied, setCopied] = useState(false);
  const toast = useToast();

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    setName(projectName);
    setDeployedUrl(null);
    setIsDeploying(false);
    setLogs([`Workspace directory: ${cwd}`, "Detecting framework and build output directory..."]);

    void (async () => {
      try {
        const detected = await invokeTauri<{ framework: string; outputDir: string }>(
          "detect_cloud_project",
          { cwd },
        );
        if (!cancelled && detected) {
          setOutputDir(detected.outputDir);
          setLogs([
            `Workspace directory: ${cwd}`,
            `Detected framework: ${detected.framework} (${detected.outputDir})`,
            "Ready to run real CLI deployment.",
          ]);
        }
      } catch {
        // Keep default dir if detection fails outside Tauri
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [visible, projectName, cwd]);

  const handleDeploy = useCallback(async () => {
    setIsDeploying(true);
    setDeployedUrl(null);
    setLogs((prev) => [...prev, `Running real ${provider} CLI deployment from ${cwd}...`]);

    try {
      const rustProvider =
        provider === "vercel"
          ? "vercel"
          : provider === "netlify"
            ? "netlify"
            : "cloudflarePages";
      const result = await invokeTauri<{
        success: boolean;
        liveUrl?: string | null;
        buildLogs: string;
        message: string;
      }>("deploy_cloud_project", {
        req: {
          cwd,
          provider: rustProvider,
          projectName: name.trim() || null,
          outputDir: outputDir.trim() || null,
        },
      });
      if (!result) {
        setLogs((prev) => [...prev, "Cloud deploy requires Kronos desktop runtime."]);
        toast.show("Cloud deploy requires desktop runtime", { variant: "error" });
        return;
      }

      const logLines = result.buildLogs
        .split(/\r?\n/)
        .map((l) => l.trimEnd())
        .filter((l) => l.length > 0);
      setLogs((prev) => [...prev, ...logLines, result.message]);

      if (result.liveUrl) {
        setDeployedUrl(result.liveUrl);
      }
      toast.show(result.message, { variant: result.success ? "success" : "error" });
    } catch (err) {
      const errMsg = `Deployment error: ${String(err)}`;
      setLogs((prev) => [...prev, errMsg]);
      toast.show(errMsg, { variant: "error" });
    } finally {
      setIsDeploying(false);
    }
  }, [provider, name, outputDir, cwd, toast]);

  const handleCopy = useCallback(() => {
    if (!deployedUrl) return;
    if (navigator?.clipboard?.writeText) {
      navigator.clipboard.writeText(deployedUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      toast.show("Deployed URL copied", { variant: "success" });
    }
  }, [deployedUrl, toast]);

  if (!visible) return null;

  return (
    <div
      data-testid="cloud-deployment-modal"
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.75)",
        backdropFilter: "blur(4px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 9999,
        padding: "16px",
      }}
    >
      <div
        style={{
          width: "580px",
          maxWidth: "100%",
          backgroundColor: "#18181b",
          border: "1px solid #27272a",
          borderRadius: "12px",
          display: "flex",
          flexDirection: "column",
          boxShadow: "0 20px 40px rgba(0, 0, 0, 0.6)",
          color: "#f4f4f5",
          fontFamily: "system-ui, -apple-system, sans-serif",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "16px 20px",
            borderBottom: "1px solid #27272a",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <div
              style={{
                width: "32px",
                height: "32px",
                borderRadius: "8px",
                backgroundColor: "rgba(59, 130, 246, 0.15)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Rocket size={18} color="#60a5fa" />
            </div>
            <div>
              <div style={{ fontWeight: 600, fontSize: "15px" }}>1-Click Cloud Deployment</div>
              <div style={{ fontSize: "12px", color: "#a1a1aa" }}>
                Deploy project directly to Vercel, Netlify, or Cloudflare
              </div>
            </div>
          </div>

          <button
            onClick={onClose}
            style={{
              background: "transparent",
              border: "none",
              color: "#a1a1aa",
              cursor: "pointer",
              padding: "4px",
            }}
          >
            <X size={18} />
          </button>
        </div>

        {/* Content */}
        <div style={{ padding: "20px", display: "flex", flexDirection: "column", gap: "16px" }}>
          {/* Provider Selection */}
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <label style={{ fontSize: "12px", fontWeight: 500, color: "#d4d4d8" }}>
              Target Provider
            </label>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "8px" }}>
              {(["vercel", "netlify", "cloudflare"] as CloudProviderId[]).map((p) => {
                const isSelected = provider === p;
                const label =
                  p === "vercel" ? "▲ Vercel" : p === "netlify" ? "◆ Netlify" : "☁ Cloudflare";
                return (
                  <button
                    key={p}
                    onClick={() => setProvider(p)}
                    style={{
                      padding: "8px 12px",
                      backgroundColor: isSelected ? "rgba(37, 99, 235, 0.2)" : "#27272a",
                      border: isSelected ? "1px solid #3b82f6" : "1px solid #3f3f46",
                      borderRadius: "6px",
                      color: isSelected ? "#93c5fd" : "#d4d4d8",
                      fontSize: "12px",
                      fontWeight: 600,
                      cursor: "pointer",
                      textAlign: "center",
                    }}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Project Name & Output Dir */}
          <div style={{ display: "flex", gap: "10px" }}>
            <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "6px" }}>
              <label style={{ fontSize: "12px", fontWeight: 500, color: "#d4d4d8" }}>
                Project Name
              </label>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                style={{
                  backgroundColor: "#27272a",
                  border: "1px solid #3f3f46",
                  borderRadius: "6px",
                  color: "#f4f4f5",
                  padding: "8px 12px",
                  fontSize: "13px",
                }}
              />
            </div>

            <div style={{ width: "120px", display: "flex", flexDirection: "column", gap: "6px" }}>
              <label style={{ fontSize: "12px", fontWeight: 500, color: "#d4d4d8" }}>
                Output Dir
              </label>
              <input
                value={outputDir}
                onChange={(e) => setOutputDir(e.target.value)}
                style={{
                  backgroundColor: "#27272a",
                  border: "1px solid #3f3f46",
                  borderRadius: "6px",
                  color: "#f4f4f5",
                  padding: "8px 12px",
                  fontSize: "13px",
                }}
              />
            </div>
          </div>

          {/* Success Banner if Deployed */}
          {deployedUrl && (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "12px 16px",
                backgroundColor: "rgba(34, 197, 94, 0.12)",
                border: "1px solid #22c55e",
                borderRadius: "8px",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <CheckCircle2 size={18} color="#22c55e" />
                <span style={{ fontSize: "13px", fontWeight: 600, color: "#4ade80" }}>
                  {deployedUrl}
                </span>
              </div>

              <div style={{ display: "flex", gap: "6px" }}>
                <button
                  onClick={handleCopy}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "4px",
                    padding: "4px 8px",
                    backgroundColor: "#27272a",
                    border: "none",
                    borderRadius: "4px",
                    color: "#fff",
                    fontSize: "11px",
                    cursor: "pointer",
                  }}
                >
                  {copied ? <Check size={12} color="#22c55e" /> : <Copy size={12} />}
                  {copied ? "Copied" : "Copy"}
                </button>
                <a
                  href={deployedUrl}
                  target="_blank"
                  rel="noreferrer"
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "4px",
                    padding: "4px 10px",
                    backgroundColor: "#16a34a",
                    border: "none",
                    borderRadius: "4px",
                    color: "#fff",
                    fontSize: "11px",
                    textDecoration: "none",
                    fontWeight: 500,
                  }}
                >
                  <ExternalLink size={12} /> Open
                </a>
              </div>
            </div>
          )}

          {/* Terminal Logs */}
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "11px", color: "#a1a1aa" }}>
              <Terminal size={12} />
              <span>Deployment Logs</span>
            </div>
            <div
              style={{
                padding: "10px 14px",
                backgroundColor: "#101012",
                border: "1px solid #27272a",
                borderRadius: "6px",
                fontFamily: "monospace",
                fontSize: "11px",
                color: "#a1a1aa",
                height: "100px",
                overflowY: "auto",
                lineHeight: "1.5",
              }}
            >
              {logs.map((line, idx) => (
                <div key={idx}>{line}</div>
              ))}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            alignItems: "center",
            padding: "12px 20px",
            borderTop: "1px solid #27272a",
            gap: "10px",
          }}
        >
          <button
            onClick={onClose}
            style={{
              padding: "6px 14px",
              backgroundColor: "transparent",
              border: "1px solid #3f3f46",
              borderRadius: "6px",
              color: "#d4d4d8",
              fontSize: "13px",
              cursor: "pointer",
            }}
          >
            {deployedUrl ? "Done" : "Cancel"}
          </button>

          {!deployedUrl && (
            <button
              onClick={() => void handleDeploy()}
              disabled={isDeploying || !name.trim()}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                padding: "6px 16px",
                backgroundColor: isDeploying ? "#3f3f46" : "#2563eb",
                border: "none",
                borderRadius: "6px",
                color: "#fff",
                fontWeight: 500,
                fontSize: "13px",
                cursor: isDeploying ? "default" : "pointer",
              }}
            >
              {isDeploying ? <Loader2 size={14} className="animate-spin" /> : <Rocket size={14} />}
              {isDeploying ? "Deploying..." : `Deploy to ${provider}`}
            </button>
          )}
        </div>
      </div>
    </div>
  );
});
