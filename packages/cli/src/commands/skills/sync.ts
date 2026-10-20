import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { Command } from "commander";
import {
  type CommandOptions,
  type OutputSchema,
  type SingleResult,
  withOutput,
} from "../../output/index.js";
import { addJsonOption } from "../../utils/command-options.js";

export const STANDARD_SKILL_NAMES = [
  "paseo-handoff.md",
  "paseo-advisor.md",
  "paseo-committee.md",
] as const;

export type StandardSkillName = (typeof STANDARD_SKILL_NAMES)[number];

export const PASEO_HANDOFF_CONTENT = `---
name: paseo-handoff
description: Hand off the current task to another agent with full context. Use when the user says "handoff", "hand off", "hand this to", or wants to pass work to another agent.
user-invocable: true
---

# Handoff Skill

Transfer the current task — context, decisions, failed attempts, constraints — to a fresh agent. The receiving agent starts with **zero context**, so the handoff prompt must be a self-contained briefing.

**User's arguments:** $ARGUMENTS

## Prerequisites

Read the **paseo** skill. Call \`list_profiles\` before choosing the receiving agent. Do not create it until you have read the configured profiles and their \`notes\`.

## Parsing arguments

1. **Agent profile** — explicit profile name first; otherwise choose the profile whose \`notes\` best match the work. Materialize it into \`create_agent\` as described by the **paseo** skill. If no profile fits, use Paseo's provider discovery fallback.
2. **Isolation** — "in a worktree" / "worktree" → create a workspace with \`isolation: "worktree"\`, using a short branch name derived from the task.
3. **Task description** — anything else the user said.

## The handoff prompt

The receiving agent has zero context. Include:

\`\`\`
## Task
[Imperative description.]

## Context
[Why this task exists, required context.]

## Relevant files
- \`path/to/file.ts\` — [what it is and why it matters]

## Current state
[What's done, what works, what doesn't.]

## What was tried
- [Approach] — [why it failed or was abandoned]

## Decisions
- [Decision — rationale]

## Acceptance criteria
- [ ] [Criterion]

## Constraints
- [Must-not / must-preserve]
\`\`\`

**Preserve task semantics.** Investigate-only → "DO NOT edit files." Fix → "implement the fix." Refactor → "refactor, not rewrite." Carry the user's exact intent.

## Launch

Prepare the handoff in a dedicated workspace:

1. Select the current workspace or call \`create_workspace\` with the requested isolation.
2. Call \`create_agent\` with a \`[Handoff] <task>\` title, the briefing as initial prompt, and the selected \`workspaceId\` when explicit placement is needed.
3. Return the agent and workspace to the user, explaining that it remains in your subagent track until they detach it manually.

Do not encode independence as a create mode and do not invoke CLI or wire-level detach operations. Detach is a user gesture in the subagents track.

Do not wait or poll for the agent to finish.
`;

export const PASEO_ADVISOR_CONTENT = `---
name: paseo-advisor
description: Spin up a single agent as an advisor — second opinion on the current task. Use when the user says "advisor", "second opinion", "what does X think", or wants an outside take without delegating the work itself.
user-invocable: true
argument-hint: "[--profile <name>] <question or topic>"
---

# Paseo Advisor

Single agent. Reads the situation you're in. Gives a judgment. You decide what to do — the advisor doesn't drive the work.

**User's request:** $ARGUMENTS

## Prerequisites

Read the **paseo** skill. Call \`list_profiles\` before choosing the advisor. Do not create the advisor until you have read the configured profiles and their \`notes\`.

## Picking the advisor

1. **User named a profile** (\`--profile UI Work\`) → select it by name.
2. **Otherwise** choose the profile whose \`notes\` best fit the question. Match the actual work: design and approach, audit and review, or research and root-cause analysis.
3. **Contrast helps.** When several profiles fit, prefer a different provider family from your own so the second opinion is genuinely fresh.

Materialize the selected profile into \`create_agent\` as described by the **paseo** skill. If no profile fits, use Paseo's provider discovery fallback.

## The briefing

The advisor has zero context. Make it self-contained:

- The question, sharply.
- What you've considered and what you've ruled out.
- Relevant files by path (don't paste — let the agent read).
- Explicit ask: "give me a recommendation, with reasoning."

End with the no-edits suffix:

\`\`\`
This is analysis only. Do NOT edit, create, or delete any files. Do NOT write code.
\`\`\`

## Forwarded skills

If \`$ARGUMENTS\` contains another skill reference — \`/unslop\`, \`/unslop-risk\`, \`$unslop\`, etc. — the user is asking the advisor to run that skill against the current task. Examples:

- \`/paseo-advisor /unslop\` → advisor runs \`/unslop\` on the current diff.
- \`/paseo-advisor /unslop-risk\` → advisor does an unslop-risk review.
- \`/paseo-advisor $diagnose this build failure\` → advisor invokes \`/diagnose\`.

Parse the forwarded skill name out of \`$ARGUMENTS\` (\`/<name>\` or \`$<name>\`). In the briefing, tell the advisor explicitly:

\`\`\`
Invoke the \`<name>\` skill against this task. Load it via the Skill tool before doing anything else.
\`\`\`

Pass through any remaining arguments after the skill name as the skill's own input. The advisor — not you — runs the skill; you're still just the orchestrator handing it the work.

## Launch and synthesize

Create the advisor agent via Paseo with a \`[Advisor] <topic>\` title and the briefing as the initial prompt. Wait for it to finish. Read its response. Synthesize for the user — the advisor's verdict + your recommendation.

## Persistent advisor

If the user wants ongoing input ("keep this advisor for the next few decisions"), don't archive after the first reply. Send follow-ups when you need another take. Archive when the user says they're done, or when the topic shifts and a fresh context would serve better.
`;

export const PASEO_COMMITTEE_CONTENT = `---
name: paseo-committee
description: Form a committee of two high-reasoning agents to step back, do root cause analysis, and produce a plan. Use when stuck, looping, tunnel-visioning, or facing a hard planning problem.
user-invocable: true
---

# Committee Skill

Two agents from contrasting profiles, fresh context, planning a solution in parallel.

**User's additional context:** $ARGUMENTS

## Prerequisites

Read the **paseo** skill. Call \`list_profiles\` before choosing committee members. Do not create committee agents until you have read the configured profiles and their \`notes\`.

Contrast is the point of a committee, so pick profiles from different provider families when possible. Materialize each profile into \`create_agent\`.

## Composition

Two members with different reasoning styles, selected from configured Agent profiles:

- one whose notes fit planning, research, or root-cause analysis
- one contrasting high-reasoning profile from another provider family

If the user names profiles, use those. If fewer than two suitable profiles are configured, use Paseo's provider discovery fallback for the missing member and tell the user. Override the selection only when the user explicitly asks for different members.

## Hard rules

- **No edits.** Every prompt to a committee member ends with the no-edits suffix:

  \`\`\`
  This is analysis only. Do NOT edit, create, or delete any files. Do NOT write code.
  \`\`\`

- **Trust the finish notification.** Do not poll, send hurry-ups, or interrupt. Models can reason for 15–30 minutes. You can go idle and Paseo will notify you.

## Workflow

1. Write a problem-level prompt
2. Create both agents in parallel via Paseo with \`[Committee] <task>\` titles and the same prompt
3. Wait for both responses
4. Resolve disagreements by passing their arguments between each other
5. Keep going until they converge into a response

Share the consensus with the user. Summarize where the agents diverged and how they resolved it.
`;

export const STANDARD_PASEO_SKILLS: Readonly<Record<StandardSkillName, string>> = {
  "paseo-handoff.md": PASEO_HANDOFF_CONTENT,
  "paseo-advisor.md": PASEO_ADVISOR_CONTENT,
  "paseo-committee.md": PASEO_COMMITTEE_CONTENT,
};

export function resolveSkillContent(
  skillName: StandardSkillName,
  customSkillsDir?: string,
): string {
  if (customSkillsDir) {
    const baseName = skillName.replace(/\.md$/, "");
    const candidate = path.join(customSkillsDir, baseName, "SKILL.md");
    if (existsSync(candidate)) {
      try {
        return readFileSync(candidate, "utf8");
      } catch {
        // Fall back to standard content
      }
    }
  }
  return STANDARD_PASEO_SKILLS[skillName];
}

export interface AgentEnvironmentInfo {
  id: "claude" | "codex" | "opencode";
  name: string;
  baseDir: string;
  targetDir: string;
  isDetected: boolean;
}

export interface DetectEnvironmentsOptions {
  home?: string;
  env?: NodeJS.ProcessEnv;
}

function isDirectory(targetPath: string): boolean {
  try {
    return statSync(targetPath).isDirectory();
  } catch {
    return false;
  }
}

export function detectAgentEnvironments(
  options?: DetectEnvironmentsOptions,
): AgentEnvironmentInfo[] {
  const env = options?.env ?? process.env;
  const home = options?.home ?? env.HOME ?? env.USERPROFILE ?? os.homedir();

  const claudeBase = env.CLAUDE_CONFIG_DIR
    ? path.resolve(env.CLAUDE_CONFIG_DIR)
    : path.join(home, ".claude");
  const codexBase = env.CODEX_HOME ? path.resolve(env.CODEX_HOME) : path.join(home, ".codex");
  const opencodeBase = env.OPENCODE_HOME
    ? path.resolve(env.OPENCODE_HOME)
    : path.join(home, ".opencode");

  const environments: Array<{
    id: "claude" | "codex" | "opencode";
    name: string;
    baseDir: string;
    targetDir: string;
  }> = [
    {
      id: "claude",
      name: "Claude Code",
      baseDir: claudeBase,
      targetDir: path.join(claudeBase, "commands"),
    },
    {
      id: "codex",
      name: "Codex CLI",
      baseDir: codexBase,
      targetDir: path.join(codexBase, "prompts"),
    },
    {
      id: "opencode",
      name: "OpenCode",
      baseDir: opencodeBase,
      targetDir: path.join(opencodeBase, "skills"),
    },
  ];

  return environments.map((e) => ({
    id: e.id,
    name: e.name,
    baseDir: e.baseDir,
    targetDir: e.targetDir,
    isDetected: isDirectory(e.baseDir) || isDirectory(e.targetDir),
  }));
}

export type SkillSyncStatus = "created" | "updated" | "skipped" | "up-to-date";

export interface SkillSyncEntry {
  skill: StandardSkillName;
  filePath: string;
  status: SkillSyncStatus;
  reason?: string;
}

export interface EnvironmentSyncResult {
  id: "claude" | "codex" | "opencode";
  name: string;
  baseDir: string;
  targetDir: string;
  skills: SkillSyncEntry[];
}

export interface SyncSkillsOptions extends DetectEnvironmentsOptions {
  force?: boolean;
  dryRun?: boolean;
  skillsDir?: string;
}

export interface SyncSkillsResult {
  dryRun: boolean;
  force: boolean;
  detectedEnvironments: EnvironmentSyncResult[];
  unconfiguredEnvironments: Array<{ id: string; name: string; targetDir: string }>;
  summary: {
    totalSynced: number;
    totalCreated: number;
    totalUpdated: number;
    totalSkipped: number;
    totalUpToDate: number;
  };
}

interface SkillSyncPlan {
  skill: StandardSkillName;
  filePath: string;
  content: string;
  force: boolean;
  dryRun: boolean;
}

function readExistingSkillContent(filePath: string): string {
  try {
    return readFileSync(filePath, "utf8");
  } catch {
    // Unreadable file, proceed with update check
    return "";
  }
}

function createSkillEntry(
  plan: SkillSyncPlan,
  status: SkillSyncStatus,
  reason?: string,
): SkillSyncEntry {
  return { skill: plan.skill, filePath: plan.filePath, status, reason };
}

function syncExistingSkill(plan: SkillSyncPlan): SkillSyncEntry {
  const existingContent = readExistingSkillContent(plan.filePath);

  if (existingContent === plan.content && !plan.force) {
    return createSkillEntry(plan, "up-to-date", "identical content");
  }

  if (plan.force) {
    if (!plan.dryRun) {
      writeFileSync(plan.filePath, plan.content, "utf8");
    }
    return createSkillEntry(plan, "updated", "forced overwrite");
  }

  return createSkillEntry(plan, "skipped", "file exists and differs (use --force to overwrite)");
}

function syncSkillFile(plan: SkillSyncPlan): SkillSyncEntry {
  if (!existsSync(plan.filePath)) {
    if (!plan.dryRun) {
      writeFileSync(plan.filePath, plan.content, "utf8");
    }
    return createSkillEntry(plan, "created");
  }
  return syncExistingSkill(plan);
}

function countStatus(totals: Record<SkillSyncStatus, number>, status: SkillSyncStatus): void {
  totals[status] += 1;
}

export function syncSkills(options: SyncSkillsOptions = {}): SyncSkillsResult {
  const { force = false, dryRun = false, skillsDir } = options;
  const allEnvironments = detectAgentEnvironments(options);
  const detected = allEnvironments.filter((env) => env.isDetected);
  const unconfigured = allEnvironments
    .filter((env) => !env.isDetected)
    .map((env) => ({ id: env.id, name: env.name, targetDir: env.targetDir }));

  const detectedResults: EnvironmentSyncResult[] = [];
  const totals: Record<SkillSyncStatus, number> = {
    created: 0,
    updated: 0,
    skipped: 0,
    "up-to-date": 0,
  };

  for (const env of detected) {
    const entries: SkillSyncEntry[] = [];

    if (!dryRun) {
      mkdirSync(env.targetDir, { recursive: true });
    }

    for (const skillName of STANDARD_SKILL_NAMES) {
      const plan: SkillSyncPlan = {
        skill: skillName,
        filePath: path.join(env.targetDir, skillName),
        content: resolveSkillContent(skillName, skillsDir),
        force,
        dryRun,
      };
      const entry = syncSkillFile(plan);
      entries.push(entry);
      countStatus(totals, entry.status);
    }

    detectedResults.push({
      id: env.id,
      name: env.name,
      baseDir: env.baseDir,
      targetDir: env.targetDir,
      skills: entries,
    });
  }

  const totalCreated = totals.created;
  const totalUpdated = totals.updated;
  const totalSkipped = totals.skipped;
  const totalUpToDate = totals["up-to-date"];

  return {
    dryRun,
    force,
    detectedEnvironments: detectedResults,
    unconfiguredEnvironments: unconfigured,
    summary: {
      totalSynced: totalCreated + totalUpdated,
      totalCreated,
      totalUpdated,
      totalSkipped,
      totalUpToDate,
    },
  };
}

export function formatSkillsSyncOutput(result: SyncSkillsResult): string {
  const lines: string[] = [];

  if (result.dryRun) {
    lines.push("[dry-run] Dry run enabled: no files or directories were written to disk.\n");
  }

  if (result.detectedEnvironments.length === 0) {
    lines.push("No supported agent environments detected.");
    lines.push("Checked locations:");
    for (const env of result.unconfiguredEnvironments) {
      lines.push(`  - ${env.name}: ${env.targetDir}`);
    }
    lines.push("\nNo skills were synced.");
    return lines.join("\n");
  }

  lines.push("Detected agent environments:");
  for (const env of result.detectedEnvironments) {
    lines.push(`  ✓ ${env.name} (${env.targetDir})`);
  }
  lines.push("");

  lines.push(result.dryRun ? "Proposed skill sync operations:" : "Synced skills:");
  for (const env of result.detectedEnvironments) {
    lines.push(`  [${env.name}]`);
    for (const skill of env.skills) {
      let icon = "✓";
      let statusText: string = skill.status;
      if (skill.status === "skipped") {
        icon = "-";
        statusText = `skipped: ${skill.reason ?? "file exists"}`;
      } else if (skill.status === "up-to-date") {
        icon = "=";
        statusText = "up-to-date";
      } else if (result.dryRun) {
        icon = "+";
        statusText = `would ${skill.status === "created" ? "create" : "update"}`;
      }
      lines.push(`    ${icon} ${skill.skill} -> ${skill.filePath} (${statusText})`);
    }
  }

  lines.push("");
  const verb = result.dryRun ? "would be synced" : "synced";
  lines.push(
    `Summary: ${result.summary.totalSynced} skills ${verb} across ${result.detectedEnvironments.length} environment(s) (${result.summary.totalSkipped} skipped, ${result.summary.totalUpToDate} up-to-date).`,
  );

  return lines.join("\n");
}

export const skillsSyncSchema: OutputSchema<SyncSkillsResult> = {
  idField: () => "skills-sync",
  columns: [],
  renderHuman: (result) => {
    if (result.type === "single") {
      return formatSkillsSyncOutput(result.data);
    }
    return "";
  },
};

export interface SkillsSyncCommandOptions extends CommandOptions {
  force?: boolean;
  dryRun?: boolean;
  home?: string;
}

export async function runSkillsSyncCommand(
  options: SkillsSyncCommandOptions,
  _command: Command,
): Promise<SingleResult<SyncSkillsResult>> {
  const result = syncSkills({
    force: Boolean(options.force),
    dryRun: Boolean(options.dryRun),
    home: typeof options.home === "string" ? options.home : undefined,
  });

  return {
    type: "single",
    data: result,
    schema: skillsSyncSchema,
  };
}

export function syncSkillsCommand(): Command {
  const command = new Command("sync")
    .description("Sync standard Paseo skills to detected agent environments")
    .option("--force", "Overwrite existing skill files")
    .option("--dry-run", "Show what would be synced without writing files")
    .option("--home <path>", "Override home directory for detection and syncing");

  addJsonOption(command);
  command.action(withOutput(runSkillsSyncCommand));
  return command;
}
