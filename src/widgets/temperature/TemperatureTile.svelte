<script lang="ts">
  import Icon from "../../lib/Icon.svelte";
  import { temps } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { WIDGETS } from "../../lib/widgets";

  const live = temps();
  const t = $derived(live.v);
  const hasTemp = $derived(t.cpuTemp != null);
  // Without LibreHardwareMonitor there are no temperatures, but CPU load still works.
  const text = $derived(
    hasTemp ? `${Math.round(t.cpuTemp!)}°` : t.cpuLoad != null ? `${Math.round(t.cpuLoad)}%` : "–",
  );
  const hot = $derived((t.cpuTemp ?? 0) >= 85 || (t.gpuTemp ?? 0) >= 85);
  const title = $derived(
    hasTemp
      ? `CPU ${Math.round(t.cpuTemp!)} °C${t.gpuTemp != null ? ` · GPU ${Math.round(t.gpuTemp)} °C` : ""}`
      : "CPU load (temperatures need LibreHardwareMonitor)",
  );

  let el: HTMLButtonElement;
  function open() {
    const s = WIDGETS.temperature.cardSize!;
    openCard("temperature", el, s.width, s.height);
  }
</script>

<button class="tile glyph" class:hot bind:this={el} onclick={open} data-nodrag {title}>
  <Icon name={hasTemp ? "thermo" : "gauge"} size="var(--glyph)" />
  <span class="value">{text}</span>
</button>

<style>
  .hot,
  .hot .value {
    color: var(--danger);
  }
  .value {
    min-width: 3ch;
    transition: color 300ms var(--ease-out);
  }
</style>
