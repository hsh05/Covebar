import { invoke } from "@tauri-apps/api/core";
import type { MenuEntry } from "./menu";

/** Shortcuts to system tools, like Windows' own Start right-click menu. Shown when Start is
 * right-clicked, or anywhere on the bar with Shift held. */
export function startMenuEntries(): MenuEntry[] {
  const run = (target: string) => () => invoke("launch_app", { target });
  return [
    { label: "Installed apps", icon: "open", action: run("ms-settings:appsfeatures") },
    { label: "Device Manager", icon: "sliders", action: run("devmgmt.msc") },
    { label: "Disk Management", icon: "folder", action: run("diskmgmt.msc") },
    { label: "Task Manager", icon: "activity", action: run("taskmgr.exe") },
    { label: "Settings", icon: "settings", action: run("ms-settings:") },
    { label: "File Explorer", icon: "folder", action: run("explorer.exe") },
    { label: "Run", icon: "open", action: run("shell:::{2559a1f3-21d7-11d4-bdaf-00c04f60b9f0}") },
  ];
}
