import { PLUGINS_LINKED } from "~/plugins/launch";
import "~/styles.css";
import { DiscordIcon, GitHubIcon } from "~/components/brand-icons";
import { useStars } from "~/routes/__root";

export function SiteHeader() {
  const { stars } = useStars();
  return (
    <header className="flex flex-col items-center gap-4 md:flex-row md:justify-between border-b border-slate-800/60 pb-4 mb-8">
      <a href="/" className="flex items-center gap-3 group">
        <img
          src="/kronos-logo.svg"
          alt="Kronos"
          className="w-8 h-8 transition-transform group-hover:scale-110 drop-shadow-[0_0_10px_rgba(6,182,212,0.5)]"
        />
        <div className="flex items-center gap-2">
          <span className="text-xl font-extrabold tracking-wider bg-gradient-to-r from-white via-slate-200 to-cyan-300 bg-clip-text text-transparent font-mono">
            KRONOS
          </span>
          <span className="rounded bg-cyan-950/80 px-1.5 py-0.5 text-[10px] font-mono font-bold text-cyan-400 border border-cyan-800/60">
            APEX
          </span>
        </div>
      </a>
      <div className="flex flex-wrap items-center justify-center gap-4">
        {PLUGINS_LINKED && (
          <a
            href="/plugins"
            className="text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            Plugins
          </a>
        )}
        <a
          href="/#compare"
          className="text-sm font-medium text-cyan-400/90 hover:text-cyan-300 transition-colors"
        >
          Kronos vs Paseo
        </a>
        <a href="/#features" className="text-sm text-slate-300 hover:text-white transition-colors">
          Features
        </a>
        <a href="/docs" className="text-sm text-slate-300 hover:text-white transition-colors">
          Docs
        </a>
        <a
          href="/#download"
          className="text-sm font-semibold text-white bg-slate-800/80 hover:bg-cyan-500 hover:text-slate-950 px-3 py-1.5 rounded-lg border border-slate-700 hover:border-cyan-400 transition-all shadow-[0_0_15px_rgba(6,182,212,0.15)]"
        >
          Download (.exe)
        </a>
        <a
          href="https://discord.gg/jz8T2uahpH"
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Discord"
          className="text-muted-foreground hover:text-foreground transition-colors inline-flex items-center"
        >
          <DiscordIcon width="18" height="18" />
        </a>
        <a
          href="https://github.com/Xenonesis/paseo"
          rel="noopener noreferrer"
          aria-label={stars ? `GitHub, ${stars} stars` : "GitHub"}
          className="text-muted-foreground hover:text-foreground transition-colors inline-flex items-center gap-1.5"
        >
          <GitHubIcon width="18" height="18" />
          {stars && <span className="text-sm">{stars}</span>}
        </a>
      </div>
    </header>
  );
}
