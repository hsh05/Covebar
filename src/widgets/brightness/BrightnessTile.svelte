<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "../../lib/Icon.svelte";
  import { displays } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { wheelLevel } from "../../lib/wheel.svelte";

  const live = displays();
  const list = $derived(live.v);

  // The wheel moves every display by the same step, relative to the first one.
  const level = wheelLevel(
    () => list[0]?.brightness ?? null,
    (v) => {
      const delta = v - (list[0]?.brightness ?? v);
      for (const d of list) {
        const next = d === list[0] ? v : Math.max(0, Math.min(100, d.brightness + delta));
        invoke("set_brightness", { id: d.id, value: next });
      }
    },
    5,
  );

  let el: HTMLButtonElement;
  function open() {
    openCard("brightness", el, 300, 76 + Math.max(1, list.length) * 64);
  }
</script>

<button
  class="tile glyph reveal"
  bind:this={el}
  onclick={open}
  onwheel={level.wheel}
  data-nodrag
  title={list.length ? `Brightness: ${level.value}% (scroll to change)` : "Brightness"}
>
  <Icon name={(level.value ?? 100) < 40 ? "sunDim" : "sun"} size="var(--glyph)" />
  {#if level.value != null}<span class="value">{level.value}%</span>{/if}
</button>
