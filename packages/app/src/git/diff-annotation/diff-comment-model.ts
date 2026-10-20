/**
 * Data model and prompt formatting utilities for interactive diff annotations
 * and line-level comments.
 */

export interface DiffAnnotation {
  /** The workspace-relative file path being annotated. */
  filePath: string;
  /** 1-based line number in the target file. */
  lineNumber: number;
  /** The surrounding diff hunk containing the annotated line. */
  diffHunk: string;
  /** The verbatim text of the line being commented on. */
  originalLineText: string;
  /** The user's feedback or instructional comment for the agent. */
  feedbackText: string;
  /** Optional diff side ("old" for deletions/base vs "new" for additions/head). */
  side?: "old" | "new";
  /** Optional unique identifier for this annotation. */
  id?: string;
  /** Optional ISO 8601 creation timestamp. */
  createdAt?: string;
}

export interface FormatDiffAnnotationOptions {
  /**
   * Whether to include the original line text if present and non-empty.
   * @default true
   */
  includeOriginalLine?: boolean;
  /**
   * Whether to include the diff hunk context if present and non-empty.
   * @default true
   */
  includeDiffHunk?: boolean;
}

/**
 * Formats a line-level diff annotation into an unambiguous, agent-ready instruction prompt.
 *
 * Example output:
 * User commented on src/index.ts at line 42: "Fix the null check here"
 *
 * Original line:
 * ```
 * const val = data.value;
 * ```
 *
 * Diff hunk:
 * ```diff
 * @@ -40,5 +40,5 @@
 * ```
 */
export function formatDiffAnnotationPrompt(
  annotation: DiffAnnotation,
  options?: FormatDiffAnnotationOptions,
): string {
  const filePath = annotation.filePath.trim();
  const lineNumber = annotation.lineNumber;
  const feedback = annotation.feedbackText.trim();

  const sections: string[] = [`User commented on ${filePath} at line ${lineNumber}: "${feedback}"`];

  const includeOriginal = options?.includeOriginalLine ?? true;
  const includeHunk = options?.includeDiffHunk ?? true;

  const originalLine = annotation.originalLineText?.trim();
  if (includeOriginal && originalLine && originalLine.length > 0) {
    sections.push(`Original line:\n\`\`\`\n${annotation.originalLineText}\n\`\`\``);
  }

  const diffHunk = annotation.diffHunk?.trim();
  if (includeHunk && diffHunk && diffHunk.length > 0) {
    sections.push(`Diff hunk:\n\`\`\`diff\n${annotation.diffHunk}\n\`\`\``);
  }

  return sections.join("\n\n");
}

export interface DiffAnnotationValidationError {
  field: keyof DiffAnnotation;
  message: string;
}

export interface DiffAnnotationValidationResult {
  isValid: boolean;
  errors: DiffAnnotationValidationError[];
}

/**
 * Validates a diff annotation object.
 *
 * Checks that:
 * - filePath is a non-empty string.
 * - lineNumber is a positive, finite integer (> 0).
 * - feedbackText is a non-empty string after trimming.
 * - side (if provided) is either "old" or "new".
 */
export function validateDiffAnnotation(
  annotation: Partial<DiffAnnotation> | null | undefined,
): DiffAnnotationValidationResult {
  const errors: DiffAnnotationValidationError[] = [];

  if (!annotation) {
    return {
      isValid: false,
      errors: [
        { field: "filePath", message: "File path is required" },
        { field: "lineNumber", message: "Line number is required" },
        { field: "feedbackText", message: "Feedback text is required" },
      ],
    };
  }

  if (typeof annotation.filePath !== "string" || annotation.filePath.trim().length === 0) {
    errors.push({ field: "filePath", message: "File path must be a non-empty string" });
  }

  if (
    typeof annotation.lineNumber !== "number" ||
    !Number.isFinite(annotation.lineNumber) ||
    !Number.isInteger(annotation.lineNumber) ||
    annotation.lineNumber <= 0
  ) {
    errors.push({ field: "lineNumber", message: "Line number must be a positive integer" });
  }

  if (typeof annotation.feedbackText !== "string" || annotation.feedbackText.trim().length === 0) {
    errors.push({ field: "feedbackText", message: "Feedback text must be a non-empty string" });
  }

  if (
    annotation.side !== undefined &&
    annotation.side !== null &&
    annotation.side !== "old" &&
    annotation.side !== "new"
  ) {
    errors.push({ field: "side", message: "Diff side must be either 'old' or 'new'" });
  }

  return {
    isValid: errors.length === 0,
    errors,
  };
}

/**
 * Type guard verifying whether an input is a valid DiffAnnotation.
 */
export function isDiffAnnotationValid(
  annotation: Partial<DiffAnnotation> | null | undefined,
): annotation is DiffAnnotation {
  return validateDiffAnnotation(annotation).isValid;
}

export interface CreateDiffAnnotationInput {
  filePath: string;
  lineNumber: number;
  diffHunk?: string;
  originalLineText?: string;
  feedbackText: string;
  side?: "old" | "new";
  id?: string;
  createdAt?: string;
}

/**
 * Helper to construct a typed DiffAnnotation with normalized defaults.
 */
export function createDiffAnnotation(input: CreateDiffAnnotationInput): DiffAnnotation {
  return {
    filePath: input.filePath.trim(),
    lineNumber: input.lineNumber,
    diffHunk: input.diffHunk ?? "",
    originalLineText: input.originalLineText ?? "",
    feedbackText: input.feedbackText.trim(),
    side: input.side,
    id: input.id ?? `diff-annotation-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`,
    createdAt: input.createdAt ?? new Date().toISOString(),
  };
}
