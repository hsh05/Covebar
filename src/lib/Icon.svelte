<script lang="ts" module>
  // Line icons on a 20x20 grid. Shapes with class "f" are filled instead of stroked.
  const BIN = "M3.5 5.5h13M8 5.5V4a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1.5M5 5.5l.8 10.6a1.5 1.5 0 0 0 1.5 1.4h5.4a1.5 1.5 0 0 0 1.5-1.4L15 5.5";
  const MIC = '<rect x="7" y="2.5" width="6" height="10" rx="3"/><path d="M4.5 9.5a5.5 5.5 0 0 0 11 0M10 15v2.5"/>';
  const SPEAKER = '<path d="M3.5 7.5h2.3L9.5 4.3v11.4l-3.7-3.2H3.5z"/>';

  export const ICONS = {
    play: '<path class="f" d="M6.5 4.3v11.4c0 .6.6.9 1.1.6l8.8-5.7a.7.7 0 0 0 0-1.2L7.6 3.7c-.5-.3-1.1 0-1.1.6z"/>',
    pause: '<rect class="f" x="5" y="4" width="3.6" height="12" rx="1.1"/><rect class="f" x="11.4" y="4" width="3.6" height="12" rx="1.1"/>',
    next: '<path class="f" d="M4 5.3v9.4c0 .5.6.8 1 .5l6.7-4.7a.6.6 0 0 0 0-1L5 4.8c-.4-.3-1 0-1 .5z"/><rect class="f" x="13.6" y="4.5" width="2.4" height="11" rx="1"/>',
    prev: '<path class="f" d="M16 5.3v9.4c0 .5-.6.8-1 .5l-6.7-4.7a.6.6 0 0 1 0-1L15 4.8c.4-.3 1 0 1 .5z"/><rect class="f" x="4" y="4.5" width="2.4" height="11" rx="1"/>',
    mic: MIC,
    micOff: MIC + '<path d="M3 3l14 14"/>',
    volume0: SPEAKER,
    volume1: SPEAKER + '<path d="M12.5 8a3 3 0 0 1 0 4"/>',
    volume2: SPEAKER + '<path d="M12.5 8a3 3 0 0 1 0 4M14.8 5.8a6.2 6.2 0 0 1 0 8.4"/>',
    volume3: SPEAKER + '<path d="M12.5 8a3 3 0 0 1 0 4M14.8 5.8a6.2 6.2 0 0 1 0 8.4M17 3.8a9 9 0 0 1 0 12.4"/>',
    volumeMute: SPEAKER + '<path d="M12.8 7.8l4.4 4.4M17.2 7.8l-4.4 4.4"/>',
    sun: '<circle cx="10" cy="10" r="3.3"/><path d="M10 2v1.8M10 16.2V18M2 10h1.8M16.2 10H18M4.3 4.3l1.3 1.3M14.4 14.4l1.3 1.3M4.3 15.7l1.3-1.3M14.4 5.6l1.3-1.3"/>',
    sunDim: '<circle cx="10" cy="10" r="3.3"/><path d="M10 3.5v.3M10 16.2v.3M3.5 10h.3M16.2 10h.3M5.4 5.4l.2.2M14.4 14.4l.2.2M5.4 14.6l.2-.2M14.4 5.6l.2-.2"/>',
    bin: `<path d="${BIN}"/>`,
    binFull: `<path d="${BIN}"/><path d="M8.3 8.5v6M11.7 8.5v6"/>`,
    hourglass: '<path d="M5.5 2.5h9M5.5 17.5h9M6.5 2.5c0 4.2 7 4 7 7.5s-7 3.3-7 7.5M13.5 2.5c0 4.2-7 4-7 7.5s7 3.3 7 7.5"/>',
    stopwatch: '<circle cx="10" cy="11.2" r="6.3"/><path d="M10 11.2V7.8M8 2.5h4M15.2 5.6l1.2-1.2"/>',
    bell: '<path d="M5 13.5V9a5 5 0 0 1 10 0v4.5l1.3 1.5H3.7zM8.2 17.2a2 2 0 0 0 3.6 0"/>',
    plus: '<path d="M10 4.5v11M4.5 10h11"/>',
    reset: '<path d="M4.2 10a5.8 5.8 0 1 0 1.7-4.1M4.2 3.6v3.5h3.5"/>',
    flag: '<path d="M5.5 17.5V3M5.5 3.5h9l-2 3.3 2 3.3h-9"/>',
    open: '<path d="M11 3.5h5.5V9M16.5 3.5l-7 7M14 11.5v4a1 1 0 0 1-1 1H4.5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h4"/>',
    check: '<path d="M4.5 10.5l3.5 3.5 7.5-8"/>',
    close: '<path d="M5 5l10 10M15 5L5 15"/>',
    sliders: '<path d="M3.5 6h7M14.5 6h2M3.5 14h2M9.5 14h7"/><circle cx="12.5" cy="6" r="2"/><circle cx="7.5" cy="14" r="2"/>',
    thermo: '<path d="M8 11.6V4.5a2 2 0 0 1 4 0v7.1a3.6 3.6 0 1 1-4 0z"/><path d="M10 13.2V8"/>',
    gauge: '<path d="M3.2 14.5a7.5 7.5 0 1 1 13.6 0"/><path d="M10 11.5l3.2-3.6"/><circle class="f" cx="10" cy="11.6" r="1.3"/>',
    coins: '<ellipse cx="8" cy="6" rx="4.8" ry="2.3"/><path d="M3.2 6v3.4c0 1.3 2.1 2.3 4.8 2.3M3.2 9.4v3.4c0 1.3 2.1 2.3 4.8 2.3"/><ellipse cx="13" cy="11.6" rx="4.3" ry="2.1"/><path d="M8.7 11.6v3c0 1.2 1.9 2.1 4.3 2.1s4.3-.9 4.3-2.1v-3"/>',
    swap: '<path d="M4 7h11.5M12.5 4l3 3-3 3M16 13H4.5M7.5 10l-3 3 3 3"/>',
    chevronUp: '<path d="M5.5 12.5L10 8l4.5 4.5"/>',
    star: '<path d="M10 3l2.1 4.4 4.8.6-3.5 3.3.9 4.8L10 13.8l-4.3 2.3.9-4.8L3.1 8l4.8-.6z"/>',
    starFill: '<path class="f" d="M10 3l2.1 4.4 4.8.6-3.5 3.3.9 4.8L10 13.8l-4.3 2.3.9-4.8L3.1 8l4.8-.6z"/>',
    tray: '<rect x="2.5" y="11" width="15" height="5.5" rx="1.5"/><path d="M6 13.8h.01M10 13.8h.01M14 13.8h.01M10 8.5V3.5M7.5 6L10 8.5 12.5 6"/>',
    music: '<path d="M7.5 15V4.5l9-2V13"/><circle cx="5.5" cy="15" r="2"/><circle cx="14.5" cy="13" r="2"/>',
  } as const;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  let { name, size = 20, class: cls = "" }: { name: IconName; size?: number | string; class?: string } = $props();
  const px = $derived(typeof size === "number" ? `${size}px` : size);
</script>

<svg class="icon {cls}" viewBox="0 0 20 20" style:width={px} style:height={px} aria-hidden="true">
  {@html ICONS[name]}
</svg>

<style>
  .icon {
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    overflow: visible;
  }
  .icon :global(.f) {
    fill: currentColor;
    stroke: none;
  }
</style>
