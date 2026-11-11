/**
 * Kronos Composer @ Mentions Resolver
 *
 * Resolves inline @ mentions (@git:diff, @git:status, @terminal:last-output, @file:...)
 * into fully hydrated context blocks prior to agent submission.
 */

export interface MentionContextPayloads {
  gitDiff?: string;
  gitStatus?: string;
  terminalOutput?: string;
}

export function hasMentions(text: string): boolean {
  return (
    text.includes("@git:diff") ||
    text.includes("@git:status") ||
    text.includes("@terminal:last-output") ||
    text.includes("@file:")
  );
}

export function resolvePromptMentions(
  prompt: string,
  payloads: MentionContextPayloads = {},
): string {
  let resolved = prompt;

  if (resolved.includes("@git:diff") && payloads.gitDiff) {
    const diffBlock = `\n\n[Attached Context: Uncommitted Git Diff]\n\`\`\`diff\n${payloads.gitDiff.trim()}\n\`\`\`\n`;
    resolved = resolved.replace(/@git:diff\b/g, diffBlock);
  }

  if (resolved.includes("@git:status") && payloads.gitStatus) {
    const statusBlock = `\n\n[Attached Context: Git Working Tree Status]\n\`\`\`\n${payloads.gitStatus.trim()}\n\`\`\`\n`;
    resolved = resolved.replace(/@git:status\b/g, statusBlock);
  }

  if (resolved.includes("@terminal:last-output") && payloads.terminalOutput) {
    const termBlock = `\n\n[Attached Context: Terminal Output]\n\`\`\`\n${payloads.terminalOutput.trim()}\n\`\`\`\n`;
    resolved = resolved.replace(/@terminal:last-output\b/g, termBlock);
  }

  return resolved;
}
