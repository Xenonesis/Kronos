// Small shared bits for the mobile mockups. The Paseo butterfly mark is the
// app icon (packages/website/public/favicon.svg) with the black plate dropped,
// so it can be tinted and placed on any tile. Tile fills are the app's identity
// palette — packages/app/src/styles/identity-colors.ts — surfaced as
// `--color-mock-tile-*` tokens in styles.css.

export function PaseoMark({ size = 20, className }: { size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 512 512"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <g id="kronosEmblem">
        <polygon points="126,90 190,90 190,400 126,400" fill="currentColor" />
        <polygon points="190,90 210,110 210,380 190,400" fill="#334155" />
        <polygon points="214,236 348,90 388,124 266,260" fill="currentColor" />
        <polygon points="246,236 384,374 346,406 208,268" fill="currentColor" opacity="0.85" />
        <polygon points="210,218 238,246 210,274 186,246" fill="#00f0ff" />
        <polygon points="210,228 226,246 210,264 196,246" fill="#ffffff" />
      </g>
    </svg>
  );
}

/** A hashed identity tile — one letter on a muted identity fill. */
export function LetterTile({
  letter,
  tone,
}: {
  letter: string;
  tone: "red" | "violet" | "amber" | "teal";
}) {
  return (
    <span
      className={`flex size-[18px] shrink-0 items-center justify-center rounded-[5px] text-[10px] font-semibold text-white ${TILE_TONE[tone]}`}
    >
      {letter}
    </span>
  );
}

const TILE_TONE = {
  red: "bg-mock-tile-red",
  violet: "bg-mock-tile-violet",
  amber: "bg-mock-tile-amber",
  teal: "bg-mock-tile-teal",
} as const;

/** The Paseo project tile — white butterfly on the black app plate. */
export function PaseoTile() {
  return (
    <span className="flex size-[18px] shrink-0 items-center justify-center rounded-[5px] bg-black text-white">
      <PaseoMark size={14} />
    </span>
  );
}
