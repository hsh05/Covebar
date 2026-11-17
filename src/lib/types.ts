export type Edge = "bottom" | "top";
export type BarSize = "small" | "large";
export type ThemePref = "system" | "dark" | "light";
export type Material = "acrylic" | "mica" | "glass";
export type Align = "center" | "left";
export type Shape = "pill" | "full";

export interface WidgetEntry {
  id: string;
  enabled: boolean;
}

export interface Settings {
  edge: Edge;
  size: BarSize;
  theme: ThemePref;
  material: Material;
  align: Align;
  shape: Shape;
  clock24h: boolean;
  clockSeconds: boolean;
  weekStartsMonday: boolean;
  widgets: WidgetEntry[];
  autostart: boolean;
  hideTaskbar: boolean;
  autoHide: boolean;
  hotkeys: Hotkeys;
  timerPresets: number[];
  currencyBase: string;
  currencyQuote: string;
  currencyFavorites: string[];
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
  location: string | null;
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

// ---- Phase 2 widgets

export interface Hotkeys {
  micMute: string;
  speakerMute: string;
}

export type HotkeyState = "ok" | "off" | "invalid" | "taken";
export interface HotkeyStatus {
  micMute: HotkeyState;
  speakerMute: HotkeyState;
}

export interface MediaInfo {
  source: string;
  sourceName: string;
  sourceIcon: string;
  title: string;
  artist: string;
  album: string;
  playing: boolean;
  canPrev: boolean;
  canNext: boolean;
  canPlayPause: boolean;
  canSeek: boolean;
  position: number;
  duration: number;
  updatedAt: number;
  art: string | null;
}

export interface Endpoint {
  id: string;
  name: string;
  volume: number;
  muted: boolean;
}

export interface AudioState {
  output: Endpoint | null;
  input: Endpoint | null;
  outputs: { id: string; name: string }[];
}

export interface DisplayInfo {
  id: string;
  name: string;
  brightness: number;
  method: "wmi" | "ddc";
}

export interface BinInfo {
  items: number;
  bytes: number;
}

export interface TimerState {
  durationMs: number;
  endsAt: number | null;
  remainingMs: number | null;
  ringing: boolean;
}

export interface StopwatchState {
  startedAt: number | null;
  elapsedMs: number;
  laps: number[];
}

export interface TimersState {
  timer: TimerState;
  stopwatch: StopwatchState;
}

export interface TrayIcon {
  id: string;
  tooltip: string;
  image: string | null;
  iconKey: string | null;
}

export interface TrayState {
  icons: TrayIcon[];
  hasOverflow: boolean;
  unavailable: boolean;
}

export interface Temps {
  /** "wmi" or "web" when LibreHardwareMonitor answered, "" when it isn't running. */
  source: "wmi" | "web" | "";
  cpuTemp: number | null;
  gpuTemp: number | null;
  cpuLoad: number | null;
  gpuLoad: number | null;
  memLoad: number | null;
  cpuName: string | null;
  gpuName: string | null;
}

export interface Rates {
  rates: Record<string, number>;
  updated: number;
  source: string;
}

export interface CurrencyState {
  rates: Rates | null;
  error: string | null;
  loading: boolean;
}

export interface UsageWindow {
  key: "session" | "week" | "opus" | "sonnet";
  label: string;
  percent: number;
  resetsAt: string | null;
}

export type UsageStatus = "ok" | "loading" | "signedOut" | "expired" | "offline" | "error";

export interface ClaudeUsage {
  status: UsageStatus;
  plan: string | null;
  windows: UsageWindow[];
  fetchedAt: number | null;
  message: string | null;
}
