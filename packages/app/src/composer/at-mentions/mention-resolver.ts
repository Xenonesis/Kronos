/**
 * Kronos Composer @ Mentions Resolver
 *
 * Resolves inline @ mentions (@git:diff, @git:status, @terminal:last-output, @file:..., @folder:...)
 * into fully hydrated context blocks prior to agent submission.
 */

export interface MentionContextPayloads {
  gitDiff?: string;
  gitStatus?: string;
  terminalOutput?: string;
}

export interface ResolveMentionsOptions {
  cwd?: string;
  client?: {
    readFile?: (
      cwd: string,
      path: string,
      requestId?: string,
      maxBytes?: number,
    ) => Promise<{ bytes: Uint8Array }>;
    getDirectorySuggestions?: (options: {
      cwd?: string;
      query: string;
      limit?: number;
      includeFiles?: boolean;
      includeDirectories?: boolean;
    }) => Promise<{ entries?: Array<{ path: string; kind: string }>; directories?: string[] }>;
  } | null;
  payloads?: MentionContextPayloads;
}

export function hasMentions(text: string): boolean {
  return (
    text.includes("@git:diff") ||
    text.includes("@git:status") ||
    text.includes("@terminal:last-output") ||
    text.includes("@file:") ||
    text.includes("@folder:")
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

async function resolveFileMentions(text: string, options: ResolveMentionsOptions): Promise<string> {
  let resolved = text;
  const fileRegex = /@file:(?:"([^"]+)"|([^\s]+))/g;
  const fileMatches = Array.from(resolved.matchAll(fileRegex));

  if (fileMatches.length === 0 || !options.client?.readFile || !options.cwd) {
    return resolved;
  }

  for (const match of fileMatches) {
    const fullMatch = match[0];
    const filePath = match[1] || match[2];
    try {
      const fileResult = await options.client.readFile(
        options.cwd,
        filePath,
        undefined,
        128 * 1024,
      );
      const content = new TextDecoder().decode(fileResult.bytes);
      const ext = filePath.split(".").pop()?.toLowerCase() || "";
      const fileBlock = `\n\n[Attached Context: file:${filePath}]\n\`\`\`${ext}\n${content.trim()}\n\`\`\`\n`;
      resolved = resolved.replace(fullMatch, fileBlock);
    } catch {
      const fallbackBlock = `\n\n[Attached Context: file:${filePath} (File reference preserved)]\n`;
      resolved = resolved.replace(fullMatch, fallbackBlock);
    }
  }

  return resolved;
}

function formatDirectoryEntries(
  entries?: Array<{ path: string; kind: string }>,
  directories?: string[],
): string {
  if (entries && entries.length > 0) {
    return entries.map((e) => `- ${e.path} (${e.kind})`).join("\n");
  }
  if (directories && directories.length > 0) {
    return directories.map((d) => `- ${d}/`).join("\n");
  }
  return "- (No files found or empty folder)";
}

async function resolveFolderMentions(
  text: string,
  options: ResolveMentionsOptions,
): Promise<string> {
  let resolved = text;
  const folderRegex = /@folder:(?:"([^"]+)"|([^\s]+))/g;
  const folderMatches = Array.from(resolved.matchAll(folderRegex));

  if (folderMatches.length === 0 || !options.client?.getDirectorySuggestions || !options.cwd) {
    return resolved;
  }

  for (const match of folderMatches) {
    const fullMatch = match[0];
    const rawFolderPath = match[1] || match[2];
    const folderPath = rawFolderPath.replace(/\/+$/, "");
    try {
      const dirResult = await options.client.getDirectorySuggestions({
        cwd: options.cwd,
        query: folderPath,
        limit: 40,
        includeFiles: true,
        includeDirectories: true,
      });
      const entriesText = formatDirectoryEntries(dirResult.entries, dirResult.directories);
      const folderBlock = `\n\n[Attached Context: directory:${folderPath}/]\n${entriesText}\n`;
      resolved = resolved.replace(fullMatch, folderBlock);
    } catch {
      const fallbackBlock = `\n\n[Attached Context: directory:${folderPath}/ (Folder reference preserved)]\n`;
      resolved = resolved.replace(fullMatch, fallbackBlock);
    }
  }

  return resolved;
}

export async function resolvePromptMentionsAsync(
  prompt: string,
  options: ResolveMentionsOptions = {},
): Promise<string> {
  const resolvedSync = resolvePromptMentions(prompt, options.payloads);
  const resolvedFiles = await resolveFileMentions(resolvedSync, options);
  return resolveFolderMentions(resolvedFiles, options);
}
