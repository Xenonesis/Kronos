import { getAlternativePages } from "~/data/alternative-pages";
import { AGENT_PAGES } from "~/data/agent-pages";
import { type Doc, getDocs } from "~/docs";

const SITE_URL = "https://xenonesis.github.io/Kronos";

const PRODUCT_PREAMBLE = `# Kronos

> The Apex AI Developer Workstation & Autonomous Agent Execution Engine.

Kronos is a high-performance native AI developer workstation built with Tauri v2 and native Rust. For agent-driven development, it provides a high-density tactical cockpit: run autonomous coding swarms, edit files, review AST diffs, run tests in sandboxed terminals, and inspect live web applications through headless browser automation.

Run many agents in parallel across isolated git worktrees. Choose a provider and model for each task, inspect the results, and verify with the frame-accurate OMP sessions timeline.

Agents run on your own machines with your existing tools, configuration, and credentials. The desktop app starts its local daemon automatically with zero Electron bloat, sub-second cold boot, and under 100MB resident memory.

Kronos supports Claude Code, Codex, GitHub Copilot, OpenCode, Pi, Antigravity, Muse Code, and additional ACP-compatible agents.

Distribution: production Windows x64 NSIS installer; web app. Source: Apache-2.0 at https://github.com/Xenonesis/Kronos. Site: https://xenonesis.github.io/Kronos/.
`;

function docLine(doc: Doc): string {
  const url = `${SITE_URL}${doc.href}.md`;
  const description = doc.frontmatter.description?.trim();
  const suffix = description ? `: ${description}` : "";
  return `- [${doc.frontmatter.title}](${url})${suffix}`;
}

function agentLine(agent: (typeof AGENT_PAGES)[number]): string {
  return `- [${agent.name}](${SITE_URL}/${agent.slug}): ${agent.subtitle}`;
}

function alternativeLine(page: ReturnType<typeof getAlternativePages>[number]): string {
  const description = page.description.trim();
  const suffix = description ? `: ${description}` : "";
  return `- [${page.title}](${SITE_URL}${page.href})${suffix}`;
}

function topLevelDocs(): Doc[] {
  return getDocs().filter((d) => !d.slug.includes("/"));
}

export function buildLlmsTxt(): string {
  const docs = topLevelDocs().map(docLine).join("\n");
  const alternatives = getAlternativePages().map(alternativeLine).join("\n");
  const agents = AGENT_PAGES.map(agentLine).join("\n");

  return `${PRODUCT_PREAMBLE}
## Docs

${docs}

## Alternatives

${alternatives}

## Supported agents

${agents}

## Optional

- [Changelog](${SITE_URL}/changelog): Release notes for the Paseo daemon, CLI, desktop, and mobile apps.
- [Download](${SITE_URL}/download): Install Paseo on Mac, Windows, Linux, iOS, Android, or run the web app.
- [Paseo Hub](${SITE_URL}/hub): Connect daemons and run GitHub, Slack, Discord, and Linear workflows through the hosted service or your own deployment.
- [Blog](${SITE_URL}/blog): Updates and technical posts from the Paseo team.
- [Privacy](${SITE_URL}/privacy): Privacy policy.
- [Terms](${SITE_URL}/terms): Terms for the official relay and hosted Hub.
- [GitHub](https://github.com/Xenonesis/Kronos): Source code, issues, and releases.
`;
}
