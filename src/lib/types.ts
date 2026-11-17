export type Edge = "bottom" | "top";
export type BarSize = "small" | "large";
export type ThemePref = "system" | "dark" | "light";
export type Material = "acrylic" | "mica";

export interface WidgetEntry {
  id: string;
  enabled: boolean;
}

export interface Settings {
  edge: Edge;
  size: BarSize;
  theme: ThemePref;
  material: Material;
  clock24h: boolean;
  clockSeconds: boolean;
  weekStartsMonday: boolean;
  widgets: WidgetEntry[];
  autostart: boolean;
  hideTaskbar: boolean;
}

export interface WinInfo {
  hwnd: number;
  title: string;
  minimized: boolean;
}

export interface DockApp {
  key: string;
  name: string;
  icon: string;
  pinned: boolean;
  launch: string | null;
  windows: WinInfo[];
}

export interface AppsSnapshot {
  apps: DockApp[];
  active: number;
}

export interface PreviewData {
  appKey: string;
  name: string;
  icon: string;
  windows: { hwnd: number; title: string }[];
}
