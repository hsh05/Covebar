<script lang="ts" module>
  import type { IconName } from "../../lib/Icon.svelte";

  export function speakerIcon(volume: number | null, muted: boolean): IconName {
    if (muted || volume == null) return "volumeMute";
    if (volume === 0) return "volume0";
    if (volume < 34) return "volume1";
    if (volume < 67) return "volume2";
    return "volume3";
  }

  /** Card height for a given number of output devices. */
  export const volumeCardHeight = (outputs: number) => 188 + (outputs > 1 ? 44 + outputs * 38 : 0);
</script>

<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "../../lib/Icon.svelte";
  import { audio } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { wheelLevel } from "../../lib/wheel.svelte";

  const live = audio();
  const out = $derived(live.v.output);

  const level = wheelLevel(
    () => (out ? Math.round(out.volume * 100) : null),
    (v) => invoke("set_volume", { which: "output", volume: v / 100 }),
    2,
  );

  let el: HTMLButtonElement;
  function open() {
    openCard("volume", el, 320, volumeCardHeight(live.v.outputs.length));
  }
  // Middle click mutes, like many tray volume tools.
  function aux(e: MouseEvent) {
    if (e.button === 1 && out) invoke("set_mute", { which: "output", muted: !out.muted });
  }
</script>

<button
  class="tile glyph reveal"
  class:muted={out?.muted}
  bind:this={el}
  onclick={open}
  onauxclick={aux}
  onwheel={level.wheel}
  data-nodrag
  title={out ? `${out.name}: ${out.muted ? "muted" : `${level.value}%`} (scroll to change, middle-click to mute)` : "No speakers"}
>
  <Icon name={speakerIcon(level.value, !!out?.muted)} size="var(--glyph)" />
  {#if out}<span class="value">{out.muted ? "Muted" : `${level.value}%`}</span>{/if}
</button>

<style>
  .muted .value {
    color: var(--text-faint);
  }
</style>
