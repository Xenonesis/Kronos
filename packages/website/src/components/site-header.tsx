import { PLUGINS_LINKED } from "~/plugins/launch";
import "~/styles.css";
import { GitHubIcon } from "~/components/brand-icons";
import { useStars } from "~/routes/__root";

export function SiteHeader() {
  const { stars } = useStars();
  return (
    <header className="flex flex-col gap-4 border border-[#262626] bg-[#121212] p-3 text-[#EAEAEA] font-mono text-xs uppercase tracking-wider md:flex-row md:items-center md:justify-between rounded-none">
      <div className="flex items-center gap-3">
        <a href="./" className="flex items-center gap-3 group">
          <span className="bg-[#FF2A2A] text-black px-1.5 py-0.5 font-bold tracking-widest text-[11px]">
            KRN
          </span>
          <span className="font-bold tracking-widest text-sm text-[#EAEAEA] group-hover:text-[#FF2A2A] transition-colors">
            KRONOS // APEX-01
          </span>
        </a>
        <span className="text-[#333333]">|</span>
        <div className="hidden sm:flex items-center gap-1.5 text-[10px] text-[#888888]">
          <span>SYS:</span>
          <span className="text-[#4AF626]">[ONLINE // 200]</span>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-3">
        {PLUGINS_LINKED && (
          <a
            href="/plugins"
            className="text-[#888888] hover:text-[#EAEAEA] hover:underline underline-offset-4 transition-colors"
          >
            [ PLUGINS ]
          </a>
        )}
        <a
          href="#compare"
          className="text-[#EAEAEA] hover:text-[#FF2A2A] transition-colors font-bold"
        >
          [ AUDIT: VS_PASEO ]
        </a>
        <a
          href="#features"
          className="text-[#888888] hover:text-[#EAEAEA] hover:underline underline-offset-4 transition-colors"
        >
          [ SPEC_SHEET ]
        </a>
        <a
          href="/docs"
          className="text-[#888888] hover:text-[#EAEAEA] hover:underline underline-offset-4 transition-colors"
        >
          [ DOCS ]
        </a>
        <a
          href="https://github.com/Xenonesis/paseo/releases/latest/download/Kronos_x64-setup.exe"
          className="bg-[#FF2A2A] text-black px-3 py-1 font-bold tracking-widest hover:bg-white transition-colors"
        >
          &gt;&gt;&gt; GET_INSTALLER.EXE
        </a>
        <a
          href="https://github.com/Xenonesis/paseo"
          target="_blank"
          rel="noopener noreferrer"
          aria-label={stars ? `GitHub, ${stars} stars` : "GitHub"}
          className="border border-[#333333] px-2 py-1 text-[#888888] hover:text-[#EAEAEA] hover:border-[#888888] transition-colors inline-flex items-center gap-1"
        >
          <GitHubIcon width="14" height="14" />
          {stars && <span className="text-[11px] font-mono">{stars}</span>}
        </a>
      </div>
    </header>
  );
}
