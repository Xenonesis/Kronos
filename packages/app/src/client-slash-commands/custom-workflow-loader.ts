/**
 * Kronos Custom Slash Command & Reusable Workflow Engine
 *
 * Provides Zed AI / Cursor Custom Prompts parity.
 * Scans `.kronos/commands/*.md` markdown files and interpolates dynamic template variables
 * ($SELECTION, $ACTIVE_FILE, $DIFF, $CLIPBOARD).
 */

export interface CustomWorkflowCommand {
  name: string;
  description: string;
  template: string;
  sourceFile: string;
}

export interface WorkflowInterpolationContext {
  selection?: string;
  activeFile?: string;
  diff?: string;
  clipboard?: string;
}

/**
 * Parses markdown file content into a CustomWorkflowCommand.
 * Supports YAML frontmatter for name and description.
 */
export function parseCustomWorkflowFile(
  filePath: string,
  fileContent: string,
): CustomWorkflowCommand {
  const normalized = fileContent.replace(/\r\n/g, "\n");
  const fileName = filePath.split("/").pop()?.split("\\").pop()?.replace(/\.md$/, "") ?? "workflow";

  let name = fileName;
  let description = `Custom workflow from ${name}.md`;
  let template = normalized;

  if (normalized.startsWith("---")) {
    const endFrontmatter = normalized.indexOf("\n---", 3);
    if (endFrontmatter !== -1) {
      const frontmatter = normalized.slice(3, endFrontmatter);
      template = normalized.slice(endFrontmatter + 4).trim();

      for (const line of frontmatter.split("\n")) {
        const colonIdx = line.indexOf(":");
        if (colonIdx !== -1) {
          const key = line.slice(0, colonIdx).trim().toLowerCase();
          const val = line
            .slice(colonIdx + 1)
            .trim()
            .replace(/^["']|["']$/g, "");
          if (key === "name" && val) name = val;
          if (key === "description" && val) description = val;
        }
      }
    }
  }

  return {
    name,
    description,
    template,
    sourceFile: filePath,
  };
}

/**
 * Interpolates variables ($SELECTION, $ACTIVE_FILE, $DIFF, $CLIPBOARD) inside workflow templates.
 */
export function interpolateWorkflowVariables(
  template: string,
  context: WorkflowInterpolationContext,
): string {
  let result = template;
  result = result.replace(/\$SELECTION/g, context.selection ?? "");
  result = result.replace(/\$ACTIVE_FILE/g, context.activeFile ?? "");
  result = result.replace(/\$DIFF/g, context.diff ?? "");
  result = result.replace(/\$CLIPBOARD/g, context.clipboard ?? "");
  return result;
}
