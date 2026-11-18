// Unit tests run in Node, without a webview: stand in for the bits of Tauri that modules touch
// when they load (event listeners, window labels). Calls into the backend resolve to null.
const g = globalThis as Record<string, unknown>;
g.window ??= globalThis;
g.__TAURI_INTERNALS__ = {
  transformCallback: () => 0,
  unregisterCallback: () => {},
  invoke: async () => null,
  metadata: { currentWindow: { label: "test" }, currentWebview: { windowLabel: "test", label: "test" } },
};
g.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
