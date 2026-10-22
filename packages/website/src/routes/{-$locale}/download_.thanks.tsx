import { createFileRoute } from "@tanstack/react-router";
import { GitHubIcon } from "~/components/brand-icons";
import { routeLocale } from "~/i18n";
import { pageMeta } from "~/meta";
import { thanksPageHref, useDesktopReleaseUrls } from "~/downloads";

export const Route = createFileRoute("/{-$locale}/download_/thanks")({
  head: ({ params }) => {
    const locale = routeLocale(params);
    return pageMeta({
      title: "Thanks for downloading Kronos",
      description: "Your Kronos download is starting. Follow development and report issues on GitHub.",
      path: "/download/thanks",
      locale,
    });
  },
  component: Thanks,
});

const COMMUNITY_LINKS = [
  {
    name: "GitHub",
    icon: <GitHubIcon className="h-6 w-6" />,
    body: "Star, report issues, contribute.",
    href: "https://github.com/Xenonesis/Kronos",
  },
];

function Thanks() {
  const urls = useDesktopReleaseUrls();

  return (
    <div className="max-w-4xl mx-auto px-6 py-20">
      <h1 className="text-3xl font-medium tracking-tight mb-4">Thanks for downloading Kronos</h1>
      <p className="text-lg text-white/70 leading-relaxed max-w-2xl">
        Get help, share feedback, and follow along.
      </p>

      <div className="mt-8 grid sm:grid-cols-3 gap-4">
        {COMMUNITY_LINKS.map((item) => (
          <a
            key={item.name}
            href={item.href}
            target="_blank"
            rel="noopener noreferrer"
            className="p-5 rounded-xl border border-white/10 bg-white/[0.02] hover:border-white/25 hover:bg-white/[0.05] transition flex flex-col justify-between space-y-4"
          >
            <div className="text-white/80">{item.icon}</div>
            <div>
              <div className="font-medium text-white">{item.name}</div>
              <div className="text-sm text-white/60 mt-1">{item.body}</div>
            </div>
          </a>
        ))}
      </div>

      <div className="mt-16 text-sm text-white/40">
        Download didn&apos;t start?{" "}
        <a href={thanksPageHref(urls.windowsExe)} className="text-white/70 underline hover:text-white">
          Try direct download
        </a>
      </div>
    </div>
  );
}
