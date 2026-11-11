import React from "react";
import { useCommandPalette } from "./use-command-palette";
import { CommandPaletteModal } from "./command-palette-modal";

export function GlobalCommandPaletteHost() {
  const { isOpen, initialMode, close, items } = useCommandPalette();
  return (
    <CommandPaletteModal isOpen={isOpen} initialMode={initialMode} onClose={close} items={items} />
  );
}
