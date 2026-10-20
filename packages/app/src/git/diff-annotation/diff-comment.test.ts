// @vitest-environment jsdom
import React from "react";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  createDiffAnnotation,
  type DiffAnnotation,
  formatDiffAnnotationPrompt,
  isDiffAnnotationValid,
  validateDiffAnnotation,
} from "./diff-comment-model";
import {
  type CompatibleDiffCommentClient,
  DiffCommentCard,
  type DiffCommentClient,
} from "./diff-comment-card";

const { theme } = vi.hoisted(() => ({
  theme: {
    spacing: { 0: 0, 1: 4, 2: 8, 3: 12, 4: 16 },
    iconSize: { sm: 14, md: 18 },
    borderWidth: { 1: 1 },
    borderRadius: { sm: 4, md: 6, lg: 8, xl: 12, "2xl": 16, full: 999 },
    fontSize: { xs: 11, sm: 13, base: 15 },
    fontWeight: { normal: "400", medium: "500", semibold: "600" },
    opacity: { 50: 0.5 },
    shadow: { md: {} },
    colors: {
      surfaceSidebar: "#111",
      surface0: "#000",
      surface1: "#111",
      surface2: "#222",
      surface3: "#333",
      foreground: "#fff",
      foregroundMuted: "#aaa",
      border: "#555",
      borderAccent: "#666",
      accent: "#0a84ff",
      accentForeground: "#fff",
      destructive: "#ff4444",
      destructiveForeground: "#fff",
      primary: "#0a84ff",
      palette: { white: "#fff" },
    },
  },
}));
vi.mock("react-native-unistyles", () => ({
  StyleSheet: {
    create: (factory: unknown) =>
      typeof factory === "function" ? (factory as (t: typeof theme) => unknown)(theme) : factory,
  },
  useUnistyles: () => ({ theme }),
  withUnistyles: (c: unknown) => c,
}));

vi.mock("lucide-react-native", () => {
  const createIcon =
    (name: string) =>
    ({ style: _style, ...props }: Record<string, unknown>) =>
      React.createElement("span", { ...props, "data-icon": name });
  return {
    AlertCircle: createIcon("AlertCircle"),
    CornerDownLeft: createIcon("CornerDownLeft"),
    MessageSquare: createIcon("MessageSquare"),
  };
});

// Exception: vitest partial module mock requires importOriginal
vi.mock("react-native", async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  return {
    ...actual,
    Pressable: ({
      children,
      onPress,
      disabled,
      testID,
      accessibilityLabel,
    }: {
      children?:
        | React.ReactNode
        | ((state: { hovered: boolean; pressed: boolean }) => React.ReactNode);
      onPress?: () => void;
      disabled?: boolean;
      testID?: string;
      accessibilityLabel?: string;
      [key: string]: unknown;
    }) => {
      const resolvedChildren =
        typeof children === "function" ? children({ hovered: false, pressed: false }) : children;
      return React.createElement(
        "button",
        {
          "aria-label": accessibilityLabel,
          "data-testid": testID,
          disabled: Boolean(disabled),
          onClick: onPress,
          type: "button",
        },
        resolvedChildren,
      );
    },
  };
});

/** Whether the validation reported an error for the given field. */
function hasErrorForField(result: { errors: Array<{ field: string }> }, field: string): boolean {
  return result.errors.some((error) => error.field === field);
}

describe("Diff Comment Model & Formatter", () => {
  describe("formatDiffAnnotationPrompt", () => {
    it("creates an unambiguous instruction for basic annotation", () => {
      const annotation: DiffAnnotation = {
        filePath: "packages/app/src/index.ts",
        lineNumber: 42,
        diffHunk: "",
        originalLineText: "",
        feedbackText: "Fix the null check here",
      };

      const result = formatDiffAnnotationPrompt(annotation);
      expect(result).toBe(
        'User commented on packages/app/src/index.ts at line 42: "Fix the null check here"',
      );
    });

    it("trims whitespace from file path and feedback text", () => {
      const annotation: DiffAnnotation = {
        filePath: "   src/utils/math.ts   ",
        lineNumber: 10,
        diffHunk: "",
        originalLineText: "",
        feedbackText: "   Simplify this calculation   \n",
      };

      const result = formatDiffAnnotationPrompt(annotation);
      expect(result).toBe(
        'User commented on src/utils/math.ts at line 10: "Simplify this calculation"',
      );
    });

    it("includes original line text when present", () => {
      const annotation: DiffAnnotation = {
        filePath: "src/server.ts",
        lineNumber: 120,
        diffHunk: "",
        originalLineText: "const port = process.env.PORT || 3000;",
        feedbackText: "Use 8080 as fallback instead",
      };

      const result = formatDiffAnnotationPrompt(annotation);
      expect(result).toContain(
        'User commented on src/server.ts at line 120: "Use 8080 as fallback instead"',
      );
      expect(result).toContain("Original line:\n```\nconst port = process.env.PORT || 3000;\n```");
    });

    it("includes diff hunk when present", () => {
      const annotation: DiffAnnotation = {
        filePath: "src/config.ts",
        lineNumber: 15,
        diffHunk: "@@ -14,3 +14,3 @@\n-const debug = false;\n+const debug = true;",
        originalLineText: "",
        feedbackText: "Do not enable debug in production",
      };

      const result = formatDiffAnnotationPrompt(annotation);
      expect(result).toContain(
        'User commented on src/config.ts at line 15: "Do not enable debug in production"',
      );
      expect(result).toContain(
        "Diff hunk:\n```diff\n@@ -14,3 +14,3 @@\n-const debug = false;\n+const debug = true;\n```",
      );
    });

    it("includes both original line text and diff hunk when both are present", () => {
      const annotation: DiffAnnotation = {
        filePath: "src/auth.ts",
        lineNumber: 77,
        diffHunk: "@@ -75,5 +75,5 @@\n-if (user.isAdmin)",
        originalLineText: "if (user.isAdmin)",
        feedbackText: "Verify token scope before checking admin role",
      };

      const result = formatDiffAnnotationPrompt(annotation);
      expect(result).toBe(
        'User commented on src/auth.ts at line 77: "Verify token scope before checking admin role"\n\n' +
          "Original line:\n```\nif (user.isAdmin)\n```\n\n" +
          "Diff hunk:\n```diff\n@@ -75,5 +75,5 @@\n-if (user.isAdmin)\n```",
      );
    });

    it("omits original line text when options.includeOriginalLine is false", () => {
      const annotation: DiffAnnotation = {
        filePath: "src/index.ts",
        lineNumber: 5,
        diffHunk: "@@ -4,3 +4,3 @@",
        originalLineText: "import { a } from 'b';",
        feedbackText: "Clean import",
      };

      const result = formatDiffAnnotationPrompt(annotation, {
        includeOriginalLine: false,
      });
      expect(result).not.toContain("Original line:");
      expect(result).toContain("Diff hunk:");
    });

    it("omits diff hunk when options.includeDiffHunk is false", () => {
      const annotation: DiffAnnotation = {
        filePath: "src/index.ts",
        lineNumber: 5,
        diffHunk: "@@ -4,3 +4,3 @@",
        originalLineText: "import { a } from 'b';",
        feedbackText: "Clean import",
      };

      const result = formatDiffAnnotationPrompt(annotation, {
        includeDiffHunk: false,
      });
      expect(result).toContain("Original line:");
      expect(result).not.toContain("Diff hunk:");
    });

    it("handles multiline feedback and special characters", () => {
      const multilineFeedback = "Please ensure:\n1. Non-null\n2. Escaped quotes: \"foo\" and 'bar'";
      const annotation: DiffAnnotation = {
        filePath: "src/parser.ts",
        lineNumber: 99,
        diffHunk: "",
        originalLineText: "",
        feedbackText: multilineFeedback,
      };

      const result = formatDiffAnnotationPrompt(annotation);
      expect(result).toBe(`User commented on src/parser.ts at line 99: "${multilineFeedback}"`);
    });
  });

  describe("validateDiffAnnotation & isDiffAnnotationValid", () => {
    it("validates a complete, valid annotation successfully", () => {
      const annotation: DiffAnnotation = {
        filePath: "src/valid.ts",
        lineNumber: 1,
        diffHunk: "@@ -1 +1 @@",
        originalLineText: "console.log('hi')",
        feedbackText: "Remove log",
        side: "new",
      };

      const result = validateDiffAnnotation(annotation);
      expect(result.isValid).toBe(true);
      expect(result.errors).toEqual([]);
      expect(isDiffAnnotationValid(annotation)).toBe(true);
    });

    it("rejects null or undefined input", () => {
      expect(validateDiffAnnotation(null).isValid).toBe(false);
      expect(validateDiffAnnotation(undefined).isValid).toBe(false);
      expect(isDiffAnnotationValid(null)).toBe(false);
    });

    it("rejects empty or whitespace-only filePath", () => {
      const result1 = validateDiffAnnotation({
        filePath: "",
        lineNumber: 1,
        feedbackText: "note",
      });
      expect(result1.isValid).toBe(false);
      expect(hasErrorForField(result1, "filePath")).toBe(true);

      const result2 = validateDiffAnnotation({
        filePath: "   ",
        lineNumber: 1,
        feedbackText: "note",
      });
      expect(result2.isValid).toBe(false);
      expect(hasErrorForField(result2, "filePath")).toBe(true);
    });

    it("rejects non-positive, non-integer, or non-finite lineNumber", () => {
      const invalidLineNumbers = [0, -5, 1.5, NaN, Infinity, -Infinity];
      for (const lineNum of invalidLineNumbers) {
        const result = validateDiffAnnotation({
          filePath: "src/file.ts",
          lineNumber: lineNum,
          feedbackText: "note",
        });
        expect(result.isValid).toBe(false);
        expect(hasErrorForField(result, "lineNumber")).toBe(true);
      }
    });

    it("rejects empty or whitespace-only feedbackText", () => {
      const result1 = validateDiffAnnotation({
        filePath: "src/file.ts",
        lineNumber: 10,
        feedbackText: "",
      });
      expect(result1.isValid).toBe(false);
      expect(hasErrorForField(result1, "feedbackText")).toBe(true);

      const result2 = validateDiffAnnotation({
        filePath: "src/file.ts",
        lineNumber: 10,
        feedbackText: "    \t\n   ",
      });
      expect(result2.isValid).toBe(false);
      expect(hasErrorForField(result2, "feedbackText")).toBe(true);
    });

    it("rejects invalid side value", () => {
      const result = validateDiffAnnotation({
        filePath: "src/file.ts",
        lineNumber: 1,
        feedbackText: "note",
        side: "middle" as unknown as "old",
      });
      expect(result.isValid).toBe(false);
      expect(hasErrorForField(result, "side")).toBe(true);
    });
  });

  describe("createDiffAnnotation helper", () => {
    it("creates a DiffAnnotation with defaults", () => {
      const created = createDiffAnnotation({
        filePath: " src/test.ts ",
        lineNumber: 25,
        feedbackText: " Refactor this function ",
      });

      expect(created.filePath).toBe("src/test.ts");
      expect(created.lineNumber).toBe(25);
      expect(created.feedbackText).toBe("Refactor this function");
      expect(created.diffHunk).toBe("");
      expect(created.originalLineText).toBe("");
      expect(created.id).toBeDefined();
      expect(created.createdAt).toBeDefined();
      expect(isDiffAnnotationValid(created)).toBe(true);
    });
  });
});

describe("DiffCommentCard Component", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  const baseAnnotation = {
    filePath: "packages/app/src/git/diff-pane.tsx",
    lineNumber: 88,
    diffHunk: "@@ -85,6 +85,7 @@",
    originalLineText: "const activeDiff = useDiffQuery();",
    side: "new" as const,
  };

  it("renders file path, line number, and original line preview", () => {
    const fakeClient: DiffCommentClient = { send: vi.fn() };
    const { getByText, getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-123",
        client: fakeClient,
        onCancel: vi.fn(),
      }),
    );

    expect(getByText("packages/app/src/git/diff-pane.tsx:88")).toBeTruthy();
    expect(getByText("head")).toBeTruthy();
    expect(getByText("const activeDiff = useDiffQuery();")).toBeTruthy();
    expect(getByTestId("diff-comment-card")).toBeTruthy();
  });

  it("disables 'Send to Agent' button when feedback text is empty", () => {
    const fakeClient: DiffCommentClient = { send: vi.fn() };
    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-123",
        client: fakeClient,
        onCancel: vi.fn(),
      }),
    );

    const sendBtn = getByTestId("diff-comment-card-send-btn");
    expect((sendBtn as HTMLButtonElement).disabled).toBe(true);
  });

  it("enables 'Send to Agent' button when feedback text is entered", () => {
    const fakeClient: DiffCommentClient = { send: vi.fn() };
    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-123",
        client: fakeClient,
        onCancel: vi.fn(),
      }),
    );
    const input = getByTestId("diff-comment-card-input");
    fireEvent.change(input, { target: { value: "Refactor this query call" } });

    const sendBtn = getByTestId("diff-comment-card-send-btn");
    expect((sendBtn as HTMLButtonElement).disabled).toBe(false);
  });

  it("submits prompt using client.send(agentId, prompt) and handles success", async () => {
    const sendMock = vi.fn().mockResolvedValue(undefined);
    const fakeClient: DiffCommentClient = { send: sendMock };
    const onSuccess = vi.fn();

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-456",
        client: fakeClient,
        onCancel: vi.fn(),
        onSuccess: onSuccess,
      }),
    );

    const input = getByTestId("diff-comment-card-input");
    fireEvent.change(input, { target: { value: "Check dependency array" } });

    const sendBtn = getByTestId("diff-comment-card-send-btn");
    await act(async () => {
      fireEvent.click(sendBtn);
    });

    expect(sendMock).toHaveBeenCalledTimes(1);
    expect(sendMock).toHaveBeenCalledWith(
      "agent-456",
      expect.stringContaining(
        'User commented on packages/app/src/git/diff-pane.tsx at line 88: "Check dependency array"',
      ),
    );
    expect(sendMock).toHaveBeenCalledWith(
      "agent-456",
      expect.stringContaining("const activeDiff = useDiffQuery();"),
    );

    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(onSuccess).toHaveBeenCalledWith(expect.stringContaining("Check dependency array"));
  });

  it("dispatches via sendAgentMessage if client only supports sendAgentMessage", async () => {
    const sendAgentMessageMock = vi.fn().mockResolvedValue(undefined);
    const client: CompatibleDiffCommentClient = {
      sendAgentMessage: sendAgentMessageMock,
    };

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-compat",
        client: client,
        onCancel: vi.fn(),
      }),
    );

    const input = getByTestId("diff-comment-card-input");
    fireEvent.change(input, { target: { value: "Fallback send" } });

    const sendBtn = getByTestId("diff-comment-card-send-btn");
    await act(async () => {
      fireEvent.click(sendBtn);
    });

    expect(sendAgentMessageMock).toHaveBeenCalledTimes(1);
    expect(sendAgentMessageMock).toHaveBeenCalledWith(
      "agent-compat",
      expect.stringContaining("Fallback send"),
    );
  });

  it("handles in-flight submission state (disables inputs and buttons)", async () => {
    let resolveSend: () => void = () => {};
    const sendPromise = new Promise<void>((resolve) => {
      resolveSend = resolve;
    });
    const sendMock = vi.fn().mockReturnValue(sendPromise);
    const fakeClient: DiffCommentClient = { send: sendMock };

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-inflight",
        client: fakeClient,
        onCancel: vi.fn(),
      }),
    );

    const input = getByTestId("diff-comment-card-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "In flight comment" } });

    const sendBtn = getByTestId("diff-comment-card-send-btn") as HTMLButtonElement;
    const cancelBtn = getByTestId("diff-comment-card-cancel-btn") as HTMLButtonElement;

    // Trigger submit
    act(() => {
      fireEvent.click(sendBtn);
    });

    // In-flight checks: buttons disabled, input non-editable (readOnly in React Native Web)
    expect(sendBtn.disabled).toBe(true);
    expect(cancelBtn.disabled).toBe(true);
    expect(input.readOnly || input.disabled).toBe(true);

    // Resolve send and wait for completion
    resolveSend();
    await act(async () => {
      await sendPromise;
    });

    expect(input.readOnly).toBe(false);
  });

  it("handles submission failure: displays error message and allows retry", async () => {
    const sendMock = vi
      .fn()
      .mockRejectedValueOnce(new Error("Network connection lost"))
      .mockResolvedValueOnce(undefined);
    const fakeClient: DiffCommentClient = { send: sendMock };
    const onError = vi.fn();
    const onSuccess = vi.fn();

    const { getByTestId, queryByTestId, getByText } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-err",
        client: fakeClient,
        onCancel: vi.fn(),
        onError: onError,
        onSuccess: onSuccess,
      }),
    );

    const input = getByTestId("diff-comment-card-input");
    fireEvent.change(input, { target: { value: "Try this fix" } });

    const sendBtn = getByTestId("diff-comment-card-send-btn");
    await act(async () => {
      fireEvent.click(sendBtn);
    });

    expect(sendMock).toHaveBeenCalledTimes(1);
    expect(onError.mock.calls[0][0]).toBeInstanceOf(Error);
    expect(onSuccess).not.toHaveBeenCalled();

    // Verify error banner is visible
    expect(getByTestId("diff-comment-card-error")).toBeTruthy();
    expect(getByText("Network connection lost")).toBeTruthy();

    // Verify button is re-enabled for retry
    expect((sendBtn as HTMLButtonElement).disabled).toBe(false);

    // Retry sending
    await act(async () => {
      fireEvent.click(sendBtn);
    });

    expect(sendMock).toHaveBeenCalledTimes(2);
    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(queryByTestId("diff-comment-card-error")).toBeNull();
  });

  it("invokes onCancel when Cancel button is clicked", () => {
    const fakeClient: DiffCommentClient = { send: vi.fn() };
    const onCancel = vi.fn();

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-123",
        client: fakeClient,
        onCancel: onCancel,
      }),
    );

    const cancelBtn = getByTestId("diff-comment-card-cancel-btn");
    fireEvent.click(cancelBtn);

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("handles Escape key to cancel", () => {
    const fakeClient: DiffCommentClient = { send: vi.fn() };
    const onCancel = vi.fn();

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-123",
        client: fakeClient,
        onCancel: onCancel,
      }),
    );

    const input = getByTestId("diff-comment-card-input");
    fireEvent.keyDown(input, { key: "Escape" });

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("handles Ctrl+Enter keyboard shortcut to submit", async () => {
    const sendMock = vi.fn().mockResolvedValue(undefined);
    const fakeClient: DiffCommentClient = { send: sendMock };

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-kbd",
        client: fakeClient,
        onCancel: vi.fn(),
      }),
    );

    const input = getByTestId("diff-comment-card-input");
    fireEvent.change(input, { target: { value: "Shortcut comment" } });

    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
    });

    expect(sendMock).toHaveBeenCalledTimes(1);
    expect(sendMock).toHaveBeenCalledWith("agent-kbd", expect.stringContaining("Shortcut comment"));
  });

  it("handles Cmd+Enter (metaKey) keyboard shortcut to submit", async () => {
    const sendMock = vi.fn().mockResolvedValue(undefined);
    const fakeClient: DiffCommentClient = { send: sendMock };

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-mac",
        client: fakeClient,
        onCancel: vi.fn(),
      }),
    );

    const input = getByTestId("diff-comment-card-input");
    fireEvent.change(input, { target: { value: "Mac shortcut comment" } });

    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter", metaKey: true });
    });

    expect(sendMock).toHaveBeenCalledTimes(1);
    expect(sendMock).toHaveBeenCalledWith(
      "agent-mac",
      expect.stringContaining("Mac shortcut comment"),
    );
  });

  it("does not submit on plain Enter without Ctrl or Cmd", async () => {
    const sendMock = vi.fn().mockResolvedValue(undefined);
    const fakeClient: DiffCommentClient = { send: sendMock };

    const { getByTestId } = render(
      React.createElement(DiffCommentCard, {
        annotation: baseAnnotation,
        agentId: "agent-enter",
        client: fakeClient,
        onCancel: vi.fn(),
      }),
    );

    const input = getByTestId("diff-comment-card-input");
    fireEvent.change(input, { target: { value: "Plain enter" } });

    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter" });
    });

    expect(sendMock).not.toHaveBeenCalled();
  });
});
