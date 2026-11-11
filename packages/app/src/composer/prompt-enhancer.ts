/**
 * Kronos Prompt Enhancer ("Magic Wand" ✨)
 *
 * Transforms concise or ambiguous user prompts into structured, highly actionable
 * engineering briefs with objectives, technical constraints, targeted areas,
 * defensive error handling, and test criteria.
 */

export interface EnhancedPromptResult {
  original: string;
  enhanced: string;
  intent: "feature" | "bugfix" | "refactor" | "test" | "general";
}

const BUGFIX_REGEX = /\b(fix|bug|error|crash|broken|issue)\b/i;
const REFACTOR_REGEX = /\b(refactor|clean up|reorganize|optimize|perf)\b/i;
const TEST_REGEX = /\b(test|unit test|e2e|mock|coverage)\b/i;
const FEATURE_REGEX = /\b(add|create|implement|build|feature|support)\b/i;

export function detectPromptIntent(
  prompt: string,
): "feature" | "bugfix" | "refactor" | "test" | "general" {
  if (BUGFIX_REGEX.test(prompt)) return "bugfix";
  if (REFACTOR_REGEX.test(prompt)) return "refactor";
  if (TEST_REGEX.test(prompt)) return "test";
  if (FEATURE_REGEX.test(prompt)) return "feature";
  return "general";
}

export function enhancePromptText(rawPrompt: string): EnhancedPromptResult {
  const trimmed = rawPrompt.trim();
  if (!trimmed) {
    return {
      original: "",
      enhanced: "",
      intent: "general",
    };
  }

  // If already structured with sections, don't double-wrap
  if (trimmed.includes("### Objective") || trimmed.includes("## Plan")) {
    return {
      original: trimmed,
      enhanced: trimmed,
      intent: detectPromptIntent(trimmed),
    };
  }

  const intent = detectPromptIntent(trimmed);

  let template = "";
  switch (intent) {
    case "bugfix":
      template = `### 🐛 Bug Fix Specification: ${trimmed}

#### 1. Root Cause Analysis
- Investigate the error symptoms and identify the exact code paths or states triggering the failure.
- Check input boundaries, edge cases, null/undefined safety, and race conditions.

#### 2. Scope & Target Files
- Inspect affected components and backend handlers associated with this issue.
- Keep modifications minimal and focused strictly on eliminating the bug without collateral regressions.

#### 3. Defensive Handling & Recovery
- Ensure graceful fallbacks and clear error logs/diagnostics when downstream operations fail.

#### 4. Verification & Testing
- Run existing test suites and verify that the fix resolves the problem cleanly with 0 errors.`;
      break;

    case "feature":
      template = `### 🚀 Feature Specification: ${trimmed}

#### 1. Architecture & Core Objective
- Implement ${trimmed} with clean separation of concerns and native platform performance.
- Ensure idiomatic integration with existing codebase patterns and state management.

#### 2. Implementation Scope
- Define required data structures, protocol types, or state models first.
- Implement business logic, error boundaries, and user-facing UI/controls.

#### 3. Edge Cases & Resilience
- Handle network/offline disconnections, invalid input states, and resource cleanup.

#### 4. Verification & Acceptance Criteria
- Verify type safety (zero TypeScript/Rust compile warnings or errors).
- Ensure existing features and automated unit tests continue to pass with 0 regressions.`;
      break;

    case "refactor":
      template = `### ♻️ Refactoring Plan: ${trimmed}

#### 1. Optimization Goals
- Streamline architecture, eliminate redundant abstractions, and reduce cyclomatic complexity.
- Maintain 100% behavioral parity with existing contracts and APIs.

#### 2. Execution Boundaries
- Cleanly separate data transformations from rendering and IO side-effects.
- Preserve all existing public exports, test coverage, and TypeScript types.

#### 3. Verification
- Run typecheck and existing test suites to confirm zero regressions.`;
      break;

    case "test":
      template = `### 🧪 Test Suite Specification: ${trimmed}

#### 1. Coverage Targets
- Write comprehensive unit and integration tests covering the happy path, boundary values, and error conditions.

#### 2. Mocking & Isolation
- Isolate IO and network boundaries using clean mocks or test fixtures.

#### 3. Assertion Rigor
- Verify exact state transitions, deterministic error propagation, and resource cleanup.`;
      break;

    default:
      template = `### 📋 Engineering Task: ${trimmed}

#### 1. Objective
- Complete the request: "${trimmed}" following best engineering practices.

#### 2. Implementation Scope
- Inspect the codebase to identify relevant files and symbols before making changes.
- Apply high-performance, maintainable code modifications adhering to existing conventions.

#### 3. Verification
- Verify that changes compile cleanly with zero errors, warnings, or broken tests.`;
      break;
  }

  return {
    original: trimmed,
    enhanced: template,
    intent,
  };
}
