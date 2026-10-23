import { getAlternativePages } from "~/data/alternative-pages";
import { AGENT_PAGES, getAgentPage } from "~/data/agent-pages";
import { type Doc, getDocs } from "~/docs";
import { getTranslator, type Locale, localizePath, withLocalePrefix } from "~/i18n";

const SITE_URL = "https://xenonesis.github.io/Kronos";

interface LlmsCopy {
  preamble: string;
  docs: string;
  alternatives: string;
  agents: string;
  optional: string;
  /** Pages under "Optional", by language-neutral path. */
  pages: { path: string; title: string; description: string }[];
}

const COPY: Record<Locale, LlmsCopy> = {
  en: {
    preamble: `# Kronos

> The Apex AI Developer Workstation & Autonomous Agent Execution Engine.

Kronos is a high-performance native AI developer workstation built with Tauri v2 and native Rust. For agent-driven development, it provides a high-density tactical cockpit: run autonomous coding swarms, edit files, review AST diffs, run tests in sandboxed terminals, and inspect live web applications through headless browser automation.

Run many agents in parallel across isolated git worktrees. Choose a provider and model for each task, inspect the results, and verify with the frame-accurate OMP sessions timeline.

Agents run on your own machines with your existing tools, configuration, and credentials. The desktop app starts its local daemon automatically with zero Electron bloat, sub-second cold boot, and under 100MB resident memory.

Kronos supports Claude Code, Codex, GitHub Copilot, OpenCode, Pi, Antigravity, Muse Code, and additional ACP-compatible agents.

Distribution: production Windows x64 NSIS installer; web app. Source: Apache-2.0 at https://github.com/Xenonesis/Kronos. Site: https://xenonesis.github.io/Kronos/.
`,
    docs: "Docs",
    alternatives: "Alternatives",
    agents: "Supported agents",
    optional: "Optional",
    pages: [
      {
        path: "/changelog",
        title: "Changelog",
        description: "Release notes for the workstation daemon, CLI, and desktop releases.",
      },
      {
        path: "/download",
        title: "Download",
        description: "Install Kronos on Windows, Linux, Mac, or run the web app.",
      },
      {
        path: "/hub",
        title: "Hub",
        description:
          "Connect daemons and run GitHub, Slack, Discord, and Linear workflows through the hosted service or your own deployment.",
      },
      {
        path: "/blog",
        title: "Blog",
        description: "Updates and technical posts from the Kronos team.",
      },
      { path: "/privacy", title: "Privacy", description: "Privacy policy." },
      {
        path: "/terms",
        title: "Terms",
        description: "Terms for the official relay and hosted Hub.",
      },
      {
        path: "https://github.com/Xenonesis/Kronos",
        title: "GitHub",
        description: "Source code, issues, and releases.",
      },
    ],
  },
  "zh-CN": {
    preamble: `# Kronos

> 顶尖 AI 开发者工作站与自主 Agent 执行引擎。

Kronos 是基于 Tauri v2 和原生 Rust 构建的高性能 AI 开发者工作站。针对 Agent 驱动的开发，它提供了高密度战术座舱：并行运行自主编程集群、编辑文件、审查 AST diff、在沙盒终端中运行测试，并通过无头浏览器自动化审查应用。

在隔离的 git worktree 中并行运行多个 Agent。为每个任务选择 Provider 和模型，检查结果，并通过高精度 OMP 会话时间线进行审查。

Agent 运行在你自己的机器上，使用你现有的工具、配置和凭据。桌面应用自动启动本地守护进程，无 Electron 冗余，亚秒级冷启动，内存占用小于 100MB。

Kronos 支持 Claude Code、Codex、GitHub Copilot、OpenCode、Pi、Antigravity、Muse Code 以及其他兼容 ACP 的 Agent。

分发方式：生产级 Windows x64 NSIS 安装程序；Web 应用。源代码：Apache-2.0，位于 https://github.com/Xenonesis/Kronos。官网：https://xenonesis.github.io/Kronos/。
`,
    docs: "文档",
    alternatives: "替代方案",
    agents: "支持的 Agent",
    optional: "可选",
    pages: [
      {
        path: "/changelog",
        title: "更新日志",
        description: "工作站 daemon、CLI 和桌面应用的发布说明。",
      },
      {
        path: "/download",
        title: "下载",
        description: "在 Windows、Linux、Mac 上安装 Kronos，或运行 Web 应用。",
      },
      {
        path: "/hub",
        title: "Hub",
        description:
          "连接 daemon，通过托管服务或你自己的部署运行 GitHub、Slack、Discord 和 Linear 工作流。",
      },
      { path: "/blog", title: "博客", description: "Kronos 团队的动态和技术文章。" },
      { path: "/privacy", title: "隐私", description: "隐私政策。" },
      { path: "/terms", title: "条款", description: "条款与服务。" },
      {
        path: "https://github.com/Xenonesis/Kronos",
        title: "GitHub",
        description: "源代码、issue 和发布版本。",
      },
    ],
  },
};

function absoluteUrl(path: string, locale: Locale): string {
  const localized = localizePath(path, locale);
  return localized.startsWith("/") ? `${SITE_URL}${localized}` : localized;
}

function docLine(doc: Doc, locale: Locale): string {
  const url = `${SITE_URL}${withLocalePrefix(doc.href, locale)}.md`;
  const description = doc.frontmatter.description?.trim();
  const suffix = description ? `: ${description}` : "";
  return `- [${doc.frontmatter.title}](${url})${suffix}`;
}

function agentLine(agent: (typeof AGENT_PAGES)[number], locale: Locale): string {
  const page = getAgentPage(agent.slug, getTranslator(locale));
  return `- [${page.name}](${absoluteUrl(`/${page.slug}`, locale)}): ${page.subtitle}`;
}

function alternativeLine(
  page: ReturnType<typeof getAlternativePages>[number],
  locale: Locale,
): string {
  const description = page.description.trim();
  const suffix = description ? `: ${description}` : "";
  return `- [${page.title}](${absoluteUrl(page.href, locale)})${suffix}`;
}

function topLevelDocs(locale: Locale): Doc[] {
  return getDocs(locale).filter((d) => !d.slug.includes("/"));
}

/** The site index for language models, in a language: `/llms.txt` or `/zh-CN/llms.txt`. */
export function buildLlmsTxt(locale: Locale): string {
  const copy = COPY[locale];
  const docs = topLevelDocs(locale)
    .map((doc) => docLine(doc, locale))
    .join("\n");
  const alternatives = getAlternativePages(locale)
    .map((page) => alternativeLine(page, locale))
    .join("\n");
  const agents = AGENT_PAGES.map((agent) => agentLine(agent, locale)).join("\n");
  const pages = copy.pages
    .map((page) => `- [${page.title}](${absoluteUrl(page.path, locale)}): ${page.description}`)
    .join("\n");

  return `${copy.preamble}
## ${copy.docs}

${docs}

## ${copy.alternatives}

${alternatives}

## ${copy.agents}

${agents}

## ${copy.optional}

${pages}
`;
}
