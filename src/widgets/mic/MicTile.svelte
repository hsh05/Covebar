<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "../../lib/Icon.svelte";
  import { audio, hotkeyStatus } from "../../lib/live.svelte";
  import { store } from "../../lib/settings.svelte";
  import { wheelLevel } from "../../lib/wheel.svelte";

  const live = audio();
  const keys = hotkeyStatus();
  const mic = $derived(live.v.input);

  const level = wheelLevel(
    () => (mic ? Math.round(mic.volume * 100) : null),
    (v) => invoke("set_volume", { which: "input", volume: v / 100 }),
    2,
  );

  const shortcut = $derived(keys.v.micMute === "ok" ? store.s?.hotkeys.micMute : "");
  const title = $derived(
    !mic
      ? "No microphone"
      : `${mic.name}: ${mic.muted ? "muted" : `on, ${level.value}%`} — click to ${mic.muted ? "unmute" : "mute"}` +
          (shortcut ? ` (${shortcut})` : ""),
  );
</script>

<button
  class="tile glyph mic"
  class:muted={mic?.muted}
  disabled={!mic}
  onclick={() => mic && invoke("set_mute", { which: "input", muted: !mic.muted })}
  onwheel={level.wheel}
  data-nodrag
  {title}
>
  <Icon name={mic?.muted || !mic ? "micOff" : "mic"} size="var(--glyph)" />
</button>

<style>
  .mic {
    transition:
      background-color var(--t-med) var(--ease-out),
      color var(--t-med) var(--ease-out);
  }
  .mic.muted {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .mic.muted:hover {
    background: color-mix(in srgb, var(--danger) 22%, transparent);
  }
  .mic:disabled {
    opacity: 0.4;
  }
</style>
