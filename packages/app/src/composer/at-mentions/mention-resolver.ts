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
    text.includes("@folder:") ||
    text.includes("@notepad") ||
    text.includes("@playbook")
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

async function resolveKronosDocsMentions(
  text: string,
  options: ResolveMentionsOptions,
): Promise<string> {
  if (!options.cwd || (!text.includes("@notepad") && !text.includes("@playbook"))) {
    return text;
  }

  let resolved = text;
  try {
    const { invokeTauri } = await import("@/utils/tauri-invoke");
    const docs = await invokeTauri<
      Array<{ kind: string; name: string; relativePath: string; content: string }>
    >("scan_kronos_workspace_docs", { cwd: options.cwd });

    if (!docs || docs.length === 0) return resolved;

    const notepadRegex = /@notepad(?::([a-zA-Z0-9_.-]+))?/g;
    resolved = resolved.replace(notepadRegex, (full, targetName: string | undefined) => {
      const notepads = docs.filter((d) => d.kind === "notepad");
      if (notepads.length === 0) return full;
      const picked = targetName
        ? notepads.filter((d) => d.name.toLowerCase() === targetName.toLowerCase())
        : notepads;
      if (picked.length === 0) return full;
      return picked
        .map(
          (d) =>
            `\n\n[Attached Context: ${d.relativePath}]\n\`\`\`markdown\n${d.content.trim()}\n\`\`\`\n`,
        )
        .join("\n");
    });

    if (resolved.includes("@playbook")) {
      const playbooks = docs.filter((d) => d.kind === "playbook");
      if (playbooks.length > 0) {
        const block = playbooks
          .map(
            (d) =>
              `\n\n[Attached Context: ${d.relativePath}]\n\`\`\`markdown\n${d.content.trim()}\n\`\`\`\n`,
          )
          .join("\n");
        resolved = resolved.replace(/@playbook\b/g, block);
      }
    }
  } catch {
    // Preserve raw mention outside desktop runtime
  }
  return resolved;
}

export async function resolvePromptMentionsAsync(
  prompt: string,
  options: ResolveMentionsOptions = {},
): Promise<string> {
  const resolvedSync = resolvePromptMentions(prompt, options.payloads);
  const resolvedFiles = await resolveFileMentions(resolvedSync, options);
  const resolvedFolders = await resolveFolderMentions(resolvedFiles, options);
  return resolveKronosDocsMentions(resolvedFolders, options);
}
