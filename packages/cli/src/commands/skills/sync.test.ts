import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Command, Option } from "commander";
import { createCli } from "../../cli.js";
import {
  detectAgentEnvironments,
  formatSkillsSyncOutput,
  PASEO_ADVISOR_CONTENT,
  PASEO_COMMITTEE_CONTENT,
  PASEO_HANDOFF_CONTENT,
  runSkillsSyncCommand,
  STANDARD_SKILL_NAMES,
  syncSkills,
  type AgentEnvironmentInfo,
  type SkillSyncEntry,
  type SkillsSyncCommandOptions,
} from "./sync.js";

function everyEnvironmentUndetected(detected: AgentEnvironmentInfo[]): boolean {
  return detected.every((env) => !env.isDetected);
}

function everyEnvironmentDetected(detected: AgentEnvironmentInfo[]): boolean {
  return detected.every((env) => env.isDetected);
}

function environmentIds(detected: AgentEnvironmentInfo[]): string[] {
  return detected.map((env) => env.id);
}

function findEnvironment(
  detected: AgentEnvironmentInfo[],
  id: AgentEnvironmentInfo["id"],
): AgentEnvironmentInfo | undefined {
  return detected.find((env) => env.id === id);
}

function allSkillsUpToDate(skills: SkillSyncEntry[] | undefined): boolean {
  return skills?.every((skill) => skill.status === "up-to-date") ?? false;
}

function findSkillEntry(skills: SkillSyncEntry[], skillName: string): SkillSyncEntry | undefined {
  return skills.find((skill) => skill.skill === skillName);
}

function findCommandByName(commands: Command[], name: string): Command | undefined {
  return commands.find((command) => command.name() === name);
}

function optionLongNames(options: Option[]): (string | undefined)[] {
  return options.map((option) => option.long);
}

describe("skills sync", () => {
  let tempHome: string;

  beforeEach(() => {
    tempHome = mkdtempSync(path.join(os.tmpdir(), "paseo-skills-sync-test-"));
  });

  afterEach(() => {
    try {
      rmSync(tempHome, { recursive: true, force: true });
    } catch {
      // Ignore cleanup error
    }
  });

  describe("environment detection", () => {
    it("reports all environments unconfigured when none exist", () => {
      const detected = detectAgentEnvironments({ home: tempHome, env: {} });
      expect(everyEnvironmentUndetected(detected)).toBe(true);
      expect(environmentIds(detected)).toEqual(["claude", "codex", "opencode"]);
    });

    it("detects Claude Code when ~/.claude exists", () => {
      mkdirSync(path.join(tempHome, ".claude"), { recursive: true });

      const detected = detectAgentEnvironments({ home: tempHome, env: {} });
      const claude = findEnvironment(detected, "claude");
      const codex = findEnvironment(detected, "codex");
      const opencode = findEnvironment(detected, "opencode");

      expect(claude?.isDetected).toBe(true);
      expect(claude?.name).toBe("Claude Code");
      expect(claude?.targetDir).toBe(path.join(tempHome, ".claude", "commands"));
      expect(codex?.isDetected).toBe(false);
      expect(opencode?.isDetected).toBe(false);
    });

    it("detects Codex CLI when ~/.codex exists", () => {
      mkdirSync(path.join(tempHome, ".codex"), { recursive: true });

      const detected = detectAgentEnvironments({ home: tempHome, env: {} });
      const codex = findEnvironment(detected, "codex");

      expect(codex?.isDetected).toBe(true);
      expect(codex?.name).toBe("Codex CLI");
      expect(codex?.targetDir).toBe(path.join(tempHome, ".codex", "prompts"));
    });

    it("detects OpenCode when ~/.opencode exists", () => {
      mkdirSync(path.join(tempHome, ".opencode"), { recursive: true });

      const detected = detectAgentEnvironments({ home: tempHome, env: {} });
      const opencode = findEnvironment(detected, "opencode");

      expect(opencode?.isDetected).toBe(true);
      expect(opencode?.name).toBe("OpenCode");
      expect(opencode?.targetDir).toBe(path.join(tempHome, ".opencode", "skills"));
    });

    it("detects environments when only the target subdirectories exist", () => {
      mkdirSync(path.join(tempHome, ".claude", "commands"), { recursive: true });
      mkdirSync(path.join(tempHome, ".codex", "prompts"), { recursive: true });
      mkdirSync(path.join(tempHome, ".opencode", "skills"), { recursive: true });

      const detected = detectAgentEnvironments({ home: tempHome, env: {} });

      expect(everyEnvironmentDetected(detected)).toBe(true);
    });

    it("respects custom environment variables", () => {
      const customClaude = path.join(tempHome, "custom-claude");
      const customCodex = path.join(tempHome, "custom-codex");
      const customOpencode = path.join(tempHome, "custom-opencode");

      mkdirSync(customClaude, { recursive: true });
      mkdirSync(customCodex, { recursive: true });
      mkdirSync(customOpencode, { recursive: true });

      const detected = detectAgentEnvironments({
        home: tempHome,
        env: {
          CLAUDE_CONFIG_DIR: customClaude,
          CODEX_HOME: customCodex,
          OPENCODE_HOME: customOpencode,
        },
      });

      const claude = findEnvironment(detected, "claude");
      const codex = findEnvironment(detected, "codex");
      const opencode = findEnvironment(detected, "opencode");

      expect(claude?.isDetected).toBe(true);
      expect(claude?.targetDir).toBe(path.join(customClaude, "commands"));
      expect(codex?.isDetected).toBe(true);
      expect(codex?.targetDir).toBe(path.join(customCodex, "prompts"));
      expect(opencode?.isDetected).toBe(true);
      expect(opencode?.targetDir).toBe(path.join(customOpencode, "skills"));
    });
  });

  describe("file generation and sync logic", () => {
    it("writes all three standard skills to detected environments", () => {
      mkdirSync(path.join(tempHome, ".claude"), { recursive: true });
      mkdirSync(path.join(tempHome, ".codex"), { recursive: true });

      const result = syncSkills({ home: tempHome, env: {} });

      expect(result.detectedEnvironments).toHaveLength(2);
      expect(result.summary.totalSynced).toBe(6);
      expect(result.summary.totalCreated).toBe(6);
      expect(result.summary.totalSkipped).toBe(0);

      // Verify Claude skills
      const claudeTargetDir = path.join(tempHome, ".claude", "commands");
      for (const skill of STANDARD_SKILL_NAMES) {
        const filePath = path.join(claudeTargetDir, skill);
        expect(existsSync(filePath)).toBe(true);
      }

      // Verify Codex skills
      const codexTargetDir = path.join(tempHome, ".codex", "prompts");
      for (const skill of STANDARD_SKILL_NAMES) {
        const filePath = path.join(codexTargetDir, skill);
        expect(existsSync(filePath)).toBe(true);
      }

      // Verify content matches
      expect(readFileSync(path.join(claudeTargetDir, "paseo-handoff.md"), "utf8")).toBe(
        PASEO_HANDOFF_CONTENT,
      );
      expect(readFileSync(path.join(claudeTargetDir, "paseo-advisor.md"), "utf8")).toBe(
        PASEO_ADVISOR_CONTENT,
      );
      expect(readFileSync(path.join(claudeTargetDir, "paseo-committee.md"), "utf8")).toBe(
        PASEO_COMMITTEE_CONTENT,
      );
    });

    it("marks already identical skills as up-to-date on second run", () => {
      mkdirSync(path.join(tempHome, ".claude"), { recursive: true });

      const firstRun = syncSkills({ home: tempHome, env: {} });
      expect(firstRun.summary.totalCreated).toBe(3);
      expect(firstRun.summary.totalSynced).toBe(3);

      const secondRun = syncSkills({ home: tempHome, env: {} });
      expect(secondRun.summary.totalCreated).toBe(0);
      expect(secondRun.summary.totalSynced).toBe(0);
      expect(secondRun.summary.totalUpToDate).toBe(3);
      expect(secondRun.summary.totalSkipped).toBe(0);

      const claude = secondRun.detectedEnvironments[0];
      expect(allSkillsUpToDate(claude?.skills)).toBe(true);
    });

    it("skips existing files that differ without --force", () => {
      const claudeCommands = path.join(tempHome, ".claude", "commands");
      mkdirSync(claudeCommands, { recursive: true });

      const customContent = "# My Custom Handoff Skill\n";
      writeFileSync(path.join(claudeCommands, "paseo-handoff.md"), customContent, "utf8");

      const result = syncSkills({ home: tempHome, env: {}, force: false });

      expect(result.summary.totalCreated).toBe(2);
      expect(result.summary.totalSkipped).toBe(1);

      // Verify original content was preserved
      expect(readFileSync(path.join(claudeCommands, "paseo-handoff.md"), "utf8")).toBe(
        customContent,
      );

      const claude = result.detectedEnvironments[0];
      const handoff = findSkillEntry(claude?.skills ?? [], "paseo-handoff.md");
      expect(handoff?.status).toBe("skipped");
      expect(handoff?.reason).toContain("--force");
    });

    it("overwrites existing files that differ when --force is true", () => {
      const claudeCommands = path.join(tempHome, ".claude", "commands");
      mkdirSync(claudeCommands, { recursive: true });

      const customContent = "# My Custom Handoff Skill\n";
      writeFileSync(path.join(claudeCommands, "paseo-handoff.md"), customContent, "utf8");

      const result = syncSkills({ home: tempHome, env: {}, force: true });

      expect(result.summary.totalCreated).toBe(2);
      expect(result.summary.totalUpdated).toBe(1);
      expect(result.summary.totalSkipped).toBe(0);
      expect(result.summary.totalSynced).toBe(3);

      // Verify content was overwritten
      expect(readFileSync(path.join(claudeCommands, "paseo-handoff.md"), "utf8")).toBe(
        PASEO_HANDOFF_CONTENT,
      );

      const claude = result.detectedEnvironments[0];
      const handoff = findSkillEntry(claude?.skills ?? [], "paseo-handoff.md");
      expect(handoff?.status).toBe("updated");
    });

    it("does not write files or directories during dry-run", () => {
      mkdirSync(path.join(tempHome, ".claude"), { recursive: true });

      const result = syncSkills({ home: tempHome, env: {}, dryRun: true });

      expect(result.dryRun).toBe(true);
      expect(result.summary.totalCreated).toBe(3);
      expect(result.summary.totalSynced).toBe(3);

      // Target directory should not have been created
      const targetDir = path.join(tempHome, ".claude", "commands");
      expect(existsSync(targetDir)).toBe(false);
    });

    it("simulates updates without writing during dry-run with force", () => {
      const claudeCommands = path.join(tempHome, ".claude", "commands");
      mkdirSync(claudeCommands, { recursive: true });

      const customContent = "# Custom Content\n";
      writeFileSync(path.join(claudeCommands, "paseo-advisor.md"), customContent, "utf8");

      const result = syncSkills({ home: tempHome, env: {}, dryRun: true, force: true });

      expect(result.dryRun).toBe(true);
      expect(result.summary.totalUpdated).toBe(1);
      expect(result.summary.totalCreated).toBe(2);

      // Verify custom content is still intact
      expect(readFileSync(path.join(claudeCommands, "paseo-advisor.md"), "utf8")).toBe(
        customContent,
      );
    });
  });

  describe("output formatting", () => {
    it("formats message when no environments are detected", () => {
      const result = syncSkills({ home: tempHome, env: {} });
      const output = formatSkillsSyncOutput(result);

      expect(output).toContain("No supported agent environments detected.");
      expect(output).toContain("Claude Code");
      expect(output).toContain("Codex CLI");
      expect(output).toContain("OpenCode");
      expect(output).toContain("No skills were synced.");
    });

    it("formats output for synced skills", () => {
      mkdirSync(path.join(tempHome, ".claude"), { recursive: true });
      const result = syncSkills({ home: tempHome, env: {} });
      const output = formatSkillsSyncOutput(result);

      expect(output).toContain("Detected agent environments:");
      expect(output).toContain("Claude Code");
      expect(output).toContain("Synced skills:");
      expect(output).toContain("paseo-handoff.md");
      expect(output).toContain("paseo-advisor.md");
      expect(output).toContain("paseo-committee.md");
      expect(output).toContain("created");
      expect(output).toContain("Summary: 3 skills synced across 1 environment(s)");
    });

    it("formats output for dry-run", () => {
      mkdirSync(path.join(tempHome, ".opencode"), { recursive: true });
      const result = syncSkills({ home: tempHome, env: {}, dryRun: true });
      const output = formatSkillsSyncOutput(result);

      expect(output).toContain("[dry-run]");
      expect(output).toContain("Proposed skill sync operations:");
      expect(output).toContain("would create");
      expect(output).toContain("3 skills would be synced");
    });

    it("formats output when files are skipped or up-to-date", () => {
      const claudeCommands = path.join(tempHome, ".claude", "commands");
      mkdirSync(claudeCommands, { recursive: true });
      writeFileSync(path.join(claudeCommands, "paseo-handoff.md"), "differing content", "utf8");
      writeFileSync(path.join(claudeCommands, "paseo-advisor.md"), PASEO_ADVISOR_CONTENT, "utf8");

      const result = syncSkills({ home: tempHome, env: {} });
      const output = formatSkillsSyncOutput(result);

      expect(output).toContain("skipped: file exists and differs (use --force to overwrite)");
      expect(output).toContain("up-to-date");
      expect(output).toContain("1 skipped, 1 up-to-date");
    });
  });

  describe("CLI integration and schema", () => {
    it("registers skills and sync commands on the CLI program", () => {
      const cli = createCli();
      const skillsCmd = findCommandByName(cli.commands, "skills");
      expect(skillsCmd).toBeDefined();

      const syncCmd = findCommandByName(skillsCmd?.commands ?? [], "sync");
      expect(syncCmd).toBeDefined();

      const options = syncCmd ? optionLongNames(syncCmd.options) : undefined;
      expect(options).toContain("--force");
      expect(options).toContain("--dry-run");
      expect(options).toContain("--home");
      expect(options).toContain("--json");
    });

    it("executes runSkillsSyncCommand and returns a SingleResult with schema", async () => {
      mkdirSync(path.join(tempHome, ".claude"), { recursive: true });

      const commandOptions: SkillsSyncCommandOptions = {
        home: tempHome,
        force: false,
        dryRun: false,
        daemonTarget: { kind: "endpoint", host: "127.0.0.1:6767" },
      };

      const cli = createCli();
      const skillsCmd = findCommandByName(cli.commands, "skills");
      const syncCmd = findCommandByName(skillsCmd?.commands ?? [], "sync");
      expect(syncCmd).toBeDefined();

      const result = await runSkillsSyncCommand(commandOptions, syncCmd!);

      expect(result.type).toBe("single");
      expect(result.data.detectedEnvironments).toHaveLength(1);
      expect(result.data.summary.totalCreated).toBe(3);

      // Verify schema renderer produces formatted text
      const rendered = result.schema.renderHuman?.(result, {
        format: "table",
        quiet: false,
        noHeaders: false,
        noColor: true,
      });
      expect(rendered).toContain("Claude Code");
      expect(rendered).toContain("paseo-handoff.md");
    });
  });
});
