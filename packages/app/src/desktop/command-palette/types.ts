export interface PaletteItem {
  id: string;
  title: string;
  subtitle?: string;
  category: "agents" | "workspaces" | "actions" | "navigation" | "files";
  shortcut?: string;
  icon?: string;
  keywords?: string[];
  run: () => void | Promise<void>;
}

export interface CommandPaletteState {
  isOpen: boolean;
  query: string;
  selectedIndex: number;
}
