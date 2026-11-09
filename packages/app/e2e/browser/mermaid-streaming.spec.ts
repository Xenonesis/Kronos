import { expect } from "@playwright/test";
import { test } from "../support/fixtures";
import {
  expectCompletedDiagram,
  expectDiagramRemainsRenderedWhileStreaming,
  expectDiagramWithLabels,
  reloadConversation,
  requestDiagram,
  waitForDiagramTurnToComplete,
} from "../support/helpers/diagram";
import { openAgentRoute, seedMockAgentWorkspace } from "../support/helpers/mock-agent";

const STREAMED_MERMAID = [
  "```mermaid",
  "flowchart LR",
  "  Start --> Middle",
  '  Middle --> Done["<i>Done</i>"]',
  "  Middle --> Review",
  "  Review --> Verify",
  "  Verify --> Ship",
  "  Ship --> Package",
  "  Package --> Sign",
  "  Sign --> Upload",
  "  Upload --> Publish",
  "  Publish --> Deploy",
  "  Deploy --> Observe",
  "  Observe --> Validate",
  "  Validate --> Announce",
  "  Announce --> Document",
  "  Document --> Archive",
  "  Archive --> Release",
  "```",
].join("\n");

test("keeps a Mermaid diagram rendered while its message streams, completes, and reloads", async ({
  page,
}) => {
  test.setTimeout(120_000);
  let diagramTurnCompleted!: Promise<void>;

  const agent = await seedMockAgentWorkspace({
    repoPrefix: "mermaid-streaming-",
    title: "Streaming Mermaid",
    featureValues: {
      mockStreamingAssistantResponse: STREAMED_MERMAID,
      mockStreamingAssistantIntervalMs: 150,
    },
  });
  try {
    await test.step("Open the conversation and request a diagram", async () => {
      await openAgentRoute(page, agent);
      await requestDiagram(agent);
      diagramTurnCompleted = waitForDiagramTurnToComplete(agent);
    });

    await test.step("The diagram renders and stays rendered while tokens arrive", async () => {
      await expectDiagramWithLabels(page, ["Start", "Middle"]);
      await expectDiagramRemainsRenderedWhileStreaming(page, diagramTurnCompleted);
    });

    await test.step("The completed diagram shows the final streamed content", async () => {
      await diagramTurnCompleted;
      await expectCompletedDiagram(page, ["Start", "Done", "Release"]);
    });

    await test.step("The completed diagram remains rendered after reload", async () => {
      await reloadConversation(page);
      await expectCompletedDiagram(page, ["Start", "Done", "Release"]);
    });
  } finally {
    await agent.cleanup();
  }
});

test("loads Mermaid only when needed and recovers from a failed runtime chunk", async ({
  page,
}) => {
  test.setTimeout(120_000);
  let runtimeRequests = 0;
  const { promise: firstRequestReleased, resolve: releaseFirstRequest } =
    Promise.withResolvers<void>();
  await page.route(/html\.gen.*\.(?:js|bundle)(?:\?|$)/, async (route) => {
    runtimeRequests += 1;
    if (runtimeRequests === 1) {
      await firstRequestReleased;
      await route.abort("failed");
    } else await route.continue();
  });
  const agent = await seedMockAgentWorkspace({
    repoPrefix: "mermaid-deferred-",
    title: "Deferred Mermaid",
    featureValues: {
      mockStreamingAssistantResponse: "```mermaid\nflowchart LR\n Start --> Done\n```",
      mockStreamingAssistantIntervalMs: 10,
    },
  });
  try {
    await openAgentRoute(page, agent);
    expect(runtimeRequests).toBe(0);
    await requestDiagram(agent);
    await waitForDiagramTurnToComplete(agent);
    await expect(page.getByTestId("mermaid-runtime-loading")).toBeVisible();
    releaseFirstRequest();
    await expect(page.getByTestId("mermaid-runtime-error")).toBeVisible();
    await page.getByTestId("mermaid-runtime-retry").click();
    await expectDiagramWithLabels(page, ["Start", "Done"]);
    await expect(page.getByTestId("mermaid-runtime-error")).toHaveCount(0);
    expect(runtimeRequests).toBe(2);

    const iframe = await page.getByTestId("mermaid-viewport").locator("iframe").elementHandle();
    let probeRequests = 0;
    await page.route("https://example.invalid/mermaid-network-probe", async (route) => {
      probeRequests += 1;
      await route.fulfill({
        status: 200,
        headers: { "access-control-allow-origin": "*" },
        body: "network must remain blocked",
      });
    });
    const frame = await iframe?.contentFrame();
    if (!frame) throw new Error("Rendered diagram frame is unavailable");
    const isolation = await frame.evaluate(async () => {
      let parentBlocked = false;
      try {
        void window.parent.document.body;
      } catch {
        parentBlocked = true;
      }
      let networkBlocked = false;
      try {
        await fetch("https://example.invalid/mermaid-network-probe");
      } catch {
        networkBlocked = true;
      }
      return { parentBlocked, networkBlocked };
    });
    expect(isolation).toEqual({ parentBlocked: true, networkBlocked: true });
    expect(probeRequests).toBe(0);
  } finally {
    releaseFirstRequest();
    await agent.cleanup();
  }
});
