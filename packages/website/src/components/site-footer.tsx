import { Link } from "@tanstack/react-router";
import { PLUGINS_LINKED } from "~/plugins/launch";
import { getAlternativePages } from "~/data/alternative-pages";
import { appStoreUrl, playStoreUrl, webAppUrl } from "~/downloads";
import { SponsorsSection } from "~/components/sponsorship";
interface SiteFooterProps {
  width?: "default" | "prose" | "wide";
  /** Pages that show the sponsors in their content turn the footer band off. */
  sponsors?: boolean;
}

const WIDTH_CLASSES: Record<NonNullable<SiteFooterProps["width"]>, string> = {
  prose: "max-w-prose p-6 md:p-12 md:pt-0",
  default: "max-w-5xl p-6 md:p-20 md:pt-0",
  wide: "max-w-7xl p-6 md:p-20 md:pt-0",
};

const CLI_PARAMS = { _splat: "cli" };
export function SiteFooter({ width = "default", sponsors = true }: SiteFooterProps) {
  const widthClasses = WIDTH_CLASSES[width];
  const alternatives = getAlternativePages();
  return (
    <footer className={`${widthClasses} mx-auto mt-12 md:mt-16`}>
      {sponsors ? <SponsorsSection placement="footer" /> : null}
      <div className="border-t border-white/10 pt-8 pb-4 grid grid-cols-2 sm:grid-cols-5 gap-8 text-sm">
        <div className="space-y-3">
          <p className="text-extra-muted-foreground">Product</p>
          <div className="space-y-2">
            <Link
              to="/blog"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Blog
            </Link>
            <Link
              to="/docs"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Docs
            </Link>
            {PLUGINS_LINKED && (
              <Link
                to="/plugins"
                className="block text-muted-foreground hover:text-foreground transition-colors"
              >
                Plugins
              </Link>
            )}
            <Link
              to="/changelog"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Changelog
            </Link>
            <Link
              to="/hub"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Hub
            </Link>
            <Link
              to="/docs/$"
              params={CLI_PARAMS}
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              CLI
            </Link>
            <Link
              to="/privacy"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Privacy
            </Link>
            <Link
              to="/terms"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Terms
            </Link>
            <Link
              to="/sponsor"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Sponsor
            </Link>
          </div>
        </div>
        <div className="space-y-3">
          <p className="text-extra-muted-foreground">Agents</p>
          <div className="space-y-2">
            <Link
              to="/claude-code"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Claude Code
            </Link>
            <Link
              to="/codex"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Codex
            </Link>
            <Link
              to="/opencode"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              OpenCode
            </Link>
            <Link
              to="/antigravity"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Antigravity
            </Link>
            <Link
              to="/muse-code"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Muse Code
            </Link>
            <Link
              to="/agents"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              All providers
            </Link>
          </div>
        </div>
        <div className="space-y-3">
          <p className="text-extra-muted-foreground">Alternatives</p>
          <div className="space-y-2">
            {alternatives.map((page) => (
              <a
                key={page.slug}
                href={`${import.meta.env.BASE_URL.replace(/\/$/, "")}${page.href}`}
                className="block text-muted-foreground hover:text-foreground transition-colors"
              >
                {page.name}
              </a>
            ))}
          </div>
        </div>
        <div className="space-y-3">
          <p className="text-extra-muted-foreground">Community</p>
          <div className="space-y-2">
            <a
              href="https://discord.gg/jz8T2uahpH"
              target="_blank"
              rel="noopener noreferrer"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Discord
            </a>
            <a
              href="https://www.reddit.com/r/PaseoAI/"
              target="_blank"
              rel="noopener noreferrer"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Reddit
            </a>
            <a
              href="https://github.com/Xenonesis/Kronos"
              target="_blank"
              rel="noopener noreferrer"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              GitHub
            </a>
          </div>
        </div>
        <div className="space-y-3">
          <p className="text-extra-muted-foreground">Download</p>
          <div className="space-y-2">
            <a
              href={appStoreUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              App Store
            </a>
            <a
              href={playStoreUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Google Play
            </a>
            <a
              href="https://github.com/Xenonesis/Kronos/releases/latest/download/Kronos_x64-setup.exe"
              target="_blank"
              rel="noopener noreferrer"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Desktop (Windows .EXE)
            </a>
            <a
              href={webAppUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="block text-muted-foreground hover:text-foreground transition-colors"
            >
              Web App
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
}
