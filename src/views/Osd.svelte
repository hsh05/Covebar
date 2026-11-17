<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import Icon, { type IconName } from "../lib/Icon.svelte";

  interface Show {
    kind: string;
    on: boolean;
    session: number;
  }

  let cur = $state<Show | null>(null);
  let out = $state(false);
  // Bumped on every show so a repeat (e.g. mute, unmute) replays the pop.
  let count = $state(0);

  listen<Show>("osd-show", (e) => {
    cur = e.payload;
    out = false;
    count++;
  });
  listen<number>("osd-out", (e) => {
    if (cur?.session === e.payload) out = true;
  });

  const look = $derived.by((): { icon: IconName; text: string } | null => {
    if (!cur) return null;
    if (cur.kind === "mic") return cur.on ? { icon: "mic", text: "Microphone on" } : { icon: "micOff", text: "Microphone muted" };
    return null;
  });
</script>

{#key count}
  {#if look && cur}
    <div class="osd" class:off={!cur.on} class:out>
      <span class="icon"><Icon name={look.icon} size={72} /></span>
      <span class="text">{look.text}</span>
    </div>
  {/if}
{/key}

<style>
  /* Always light-on-dark, like the Windows volume flyout, so it reads on any wallpaper. */
  .osd {
    position: absolute;
    inset: 16px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    border-radius: 28px;
    background: rgb(16 18 20 / 0.5);
    border: 1px solid rgb(255 255 255 / 0.12);
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.25);
    color: #fff;
    animation: osd-in 260ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .osd.out {
    animation: osd-out 220ms cubic-bezier(0.4, 0, 1, 1) both;
  }
  .icon {
    display: grid;
    animation: icon-in 360ms cubic-bezier(0.34, 1.56, 0.64, 1) 60ms both;
  }
  .off .icon {
    color: #ff6b6b;
  }
  .text {
    font: 600 16px var(--font-display, var(--font));
    letter-spacing: 0.01em;
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.4);
  }
  @keyframes osd-in {
    from {
      opacity: 0;
      transform: scale(0.86);
    }
  }
  @keyframes osd-out {
    to {
      opacity: 0;
      transform: scale(0.94);
    }
  }
  @keyframes icon-in {
    from {
      transform: scale(0.6);
    }
  }
</style>
