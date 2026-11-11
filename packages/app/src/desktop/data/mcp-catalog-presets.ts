import type { McpServerConfig } from "@/desktop/components/mcp-servers-card";

export interface McpPresetItem {
  id: string;
  name: string;
  description: string;
  category: "Filesystem & DB" | "Developer Tools" | "Search & Web" | "Communication";
  author: string;
  config: McpServerConfig;
  envRequirements?: string[];
}

export const CURATED_MCP_CATALOG: McpPresetItem[] = [
  {
    id: "github-mcp",
    name: "GitHub MCP",
    description:
      "Search repositories, read code, view issues, inspect pull requests and commit histories.",
    category: "Developer Tools",
    author: "Model Context Protocol",
    config: {
      id: "github-mcp",
      name: "GitHub MCP",
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-github"],
      enabled: true,
    },
    envRequirements: ["GITHUB_PERSONAL_ACCESS_TOKEN"],
  },
  {
    id: "postgres-mcp",
    name: "PostgreSQL MCP",
    description:
      "Inspect schemas, read tables, and safely execute read-only queries on Postgres databases.",
    category: "Filesystem & DB",
    author: "Model Context Protocol",
    config: {
      id: "postgres-mcp",
      name: "PostgreSQL MCP",
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/mydb"],
      enabled: true,
    },
  },
  {
    id: "docker-mcp",
    name: "Docker MCP",
    description:
      "Inspect running Docker containers, images, volumes, networks, and stream container logs.",
    category: "Developer Tools",
    author: "Community",
    config: {
      id: "docker-mcp",
      name: "Docker MCP",
      transport: "stdio",
      command: "npx",
      args: ["-y", "mcp-server-docker"],
      enabled: true,
    },
  },
  {
    id: "brave-search-mcp",
    name: "Brave Search MCP",
    description:
      "Real-time web search with privacy-first indexing for researching documentation and solutions.",
    category: "Search & Web",
    author: "Model Context Protocol",
    config: {
      id: "brave-search-mcp",
      name: "Brave Search MCP",
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-brave-search"],
      enabled: true,
    },
    envRequirements: ["BRAVE_API_KEY"],
  },
  {
    id: "sqlite-mcp",
    name: "SQLite MCP",
    description:
      "Connect directly to local SQLite files (.sqlite, .db) to query tables and analyze records.",
    category: "Filesystem & DB",
    author: "Model Context Protocol",
    config: {
      id: "sqlite-mcp",
      name: "SQLite MCP",
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-sqlite", "./database.sqlite"],
      enabled: true,
    },
  },
  {
    id: "puppeteer-mcp",
    name: "Puppeteer Web Automation",
    description:
      "Autonomous browser automation: navigate web apps, take screenshots, and evaluate DOM state.",
    category: "Search & Web",
    author: "Model Context Protocol",
    config: {
      id: "puppeteer-mcp",
      name: "Puppeteer Web Automation",
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-puppeteer"],
      enabled: true,
    },
  },
  {
    id: "fetch-mcp",
    name: "HTTP Web Fetch",
    description:
      "Fetches and converts raw web pages into clean, optimized markdown for agent analysis.",
    category: "Search & Web",
    author: "Model Context Protocol",
    config: {
      id: "fetch-mcp",
      name: "HTTP Web Fetch",
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-fetch"],
      enabled: true,
    },
  },
  {
    id: "slack-mcp",
    name: "Slack MCP",
    description:
      "Post messages, query channels, and notify team channels of agent build completions.",
    category: "Communication",
    author: "Model Context Protocol",
    config: {
      id: "slack-mcp",
      name: "Slack MCP",
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-slack"],
      enabled: true,
    },
    envRequirements: ["SLACK_BOT_TOKEN"],
  },
];
