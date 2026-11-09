import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it } from "vitest";
import { onlineManager, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MermaidIframeRuntime, type MermaidRenderedMessage } from "./iframe-runtime.web";
import type { MermaidRenderRequest } from "./render-model";
import { useMermaidRuntimeHtml } from "./runtime-loader.web";

let root: Root | undefined;
let container: HTMLDivElement | undefined;

afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
  root = undefined;
  container = undefined;
});

it("retains a valid streaming prefix while the optional runtime is still loading", async () => {
  container = document.createElement("div");
  container.style.cssText = "width: 640px; height: 480px";
  document.body.appendChild(container);
  root = createRoot(container);
  const rendered: MermaidRenderedMessage[] = [];
  const failed: number[] = [];
  const onRendered = (message: MermaidRenderedMessage) => rendered.push(message);
  const onRenderFailed = (revision: number) => failed.push(revision);
  const valid: MermaidRenderRequest = {
    revision: 1,
    source: "flowchart LR\n Start --> Done\n",
    colorScheme: "dark",
  };
  const incomplete: MermaidRenderRequest = {
    revision: 2,
    source: "flowchart LR\n Start --> Done\n Done --",
    colorScheme: "dark",
  };
  function update(html: string | undefined, request: MermaidRenderRequest) {
    act(() =>
      root?.render(
        <MermaidIframeRuntime
          html={html}
          request={request}
          onRendered={onRendered}
          onRenderFailed={onRenderFailed}
        />,
      ),
    );
  }
  update(undefined, valid);
  update(undefined, incomplete);
  expect(container.querySelector("iframe")).toBeNull();
  expect(rendered).toEqual([]);

  // Exercise the real loading boundary and renderer, not a stub HTML bridge.
  const { mermaidRuntimeHtml } = await import("./runtime/html.gen");
  update(mermaidRuntimeHtml, incomplete);
  await expect
    .poll(() => rendered.map((message) => message.source), { timeout: 15_000 })
    .toContain(valid.source);
  await expect.poll(() => failed, { timeout: 15_000 }).toContain(incomplete.revision);
  const iframe = container.querySelector("iframe");
  expect(() => iframe?.contentWindow?.document).toThrow();
}, 30_000);

it("loads the packaged runtime and renders while React Query considers the browser offline", async () => {
  const wasOnline = onlineManager.isOnline();
  const client = new QueryClient();
  const rendered: MermaidRenderedMessage[] = [];
  const failed: number[] = [];
  const request: MermaidRenderRequest = {
    revision: 1,
    source: "flowchart LR\n Offline --> Local\n",
    colorScheme: "dark",
  };
  const onRendered = (message: MermaidRenderedMessage) => rendered.push(message);
  const onRenderFailed = (revision: number) => failed.push(revision);
  function OfflineDiagram() {
    const runtime = useMermaidRuntimeHtml();
    return (
      <MermaidIframeRuntime
        html={runtime.data}
        request={request}
        onRendered={onRendered}
        onRenderFailed={onRenderFailed}
      />
    );
  }
  try {
    onlineManager.setOnline(false);
    container = document.createElement("div");
    container.style.cssText = "width: 640px; height: 480px";
    document.body.appendChild(container);
    root = createRoot(container);
    act(() =>
      root?.render(
        <QueryClientProvider client={client}>
          <OfflineDiagram />
        </QueryClientProvider>,
      ),
    );
    await expect
      .poll(() => rendered.map((message) => message.source), { timeout: 15_000 })
      .toEqual([request.source]);
    expect(onlineManager.isOnline()).toBe(false);
    expect(failed).toEqual([]);
  } finally {
    act(() => root?.unmount());
    root = undefined;
    client.clear();
    onlineManager.setOnline(wasOnline);
  }
}, 30_000);
