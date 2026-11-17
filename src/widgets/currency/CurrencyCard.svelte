<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fly, fade, scale } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import { tick } from "svelte";
  import Icon from "../../lib/Icon.svelte";
  import { currency, ticker } from "../../lib/live.svelte";
  import { store, updateSettings } from "../../lib/settings.svelte";
  import { ago } from "../../lib/format";
  import { currencyName, money, rate } from "./currency";

  const live = currency();
  const st = $derived(live.v);
  const base = $derived(store.s?.currencyBase ?? "USD");
  const quote = $derived(store.s?.currencyQuote ?? "AED");
  const favorites = $derived(store.s?.currencyFavorites ?? []);
  const clock = ticker(30000, () => true);

  let amount = $state("1");
  const n = $derived(Number(amount.replace(/,/g, "")));
  const r = $derived(rate(st.rates, base, quote));
  const result = $derived(r != null && Number.isFinite(n) ? n * r : null);

  // Swapping spins the button and slides the two codes past each other.
  let turns = $state(0);
  function swap() {
    turns++;
    updateSettings({ currencyBase: quote, currencyQuote: base });
  }

  // The currency picker slides over the card.
  let picking = $state<"base" | "quote" | null>(null);
  let query = $state("");
  let search = $state<HTMLInputElement>();
  const codes = $derived(Object.keys(st.rates?.rates ?? {}).sort());
  const matches = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = codes.map((c) => ({ code: c, name: currencyName(c) }));
    if (!q) return list;
    return list.filter((c) => c.code.toLowerCase().includes(q) || c.name.toLowerCase().includes(q));
  });
  async function pick(which: "base" | "quote") {
    picking = which;
    query = "";
    await tick();
    search?.focus();
  }
  function choose(code: string) {
    if (picking === "base") updateSettings({ currencyBase: code });
    else if (picking === "quote") updateSettings({ currencyQuote: code });
    picking = null;
  }
  function pickKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      picking = null;
    } else if (e.key === "Enter" && matches[0]) choose(matches[0].code);
  }

  let spinning = $state(false);
  async function refresh() {
    spinning = true;
    await invoke("refresh_currency");
    setTimeout(() => (spinning = false), 700);
  }
</script>

<div class="card">
  <div class="card-title">
    <Icon name="coins" size={16} />
    <span class="grow">Currency</span>
    <button class="icon-btn small" class:spinning={spinning || st.loading} onclick={refresh} title="Update rates" aria-label="Update rates">
      <Icon name="reset" size={14} />
    </button>
  </div>

  {#if st.rates}
    <div class="convert">
      <div class="field">
        <input
          class="amount"
          inputmode="decimal"
          bind:value={amount}
          aria-label="Amount in {base}"
          onfocus={(e) => (e.currentTarget as HTMLInputElement).select()}
        />
        {#key base}
          <button class="code" onclick={() => pick("base")} in:fly={{ y: turns ? 14 : 0, duration: 260, easing: cubicOut }}>
            {base}
          </button>
        {/key}
      </div>

      <button class="swap" onclick={swap} title="Swap" aria-label="Swap currencies" style:transform="rotate({turns * 180}deg)">
        <Icon name="swap" size={16} />
      </button>

      <div class="field out">
        <span class="result" title={result != null ? String(result) : ""}>{result != null ? money(result) : "–"}</span>
        {#key quote}
          <button class="code" onclick={() => pick("quote")} in:fly={{ y: turns ? -14 : 0, duration: 260, easing: cubicOut }}>
            {quote}
          </button>
        {/key}
      </div>
      <div class="rate muted-text">
        <span>1 {base} = {r != null ? money(r) : "–"} {quote} · {currencyName(quote)}</span>
      </div>
    </div>

    {#if favorites.length}
      <div class="favs">
        {#each favorites as c (c)}
          {@const fr = rate(st.rates, base, c)}
          <button
            class="fav"
            class:active={c === quote}
            onclick={() => updateSettings({ currencyQuote: c })}
            title={currencyName(c)}
            animate:flip={{ duration: 240, easing: cubicOut }}
            transition:scale={{ duration: 180, start: 0.8, easing: cubicOut }}
          >
            <span class="fcode">{c}</span>
            <span class="fval">{fr != null ? money(fr) : "–"}</span>
          </button>
        {/each}
      </div>
    {/if}

    <div class="foot muted-text">
      {#if st.error}
        <span class="warn" in:fade>Offline · showing rates from {ago(st.rates.updated, clock.now)}</span>
      {:else}
        Rates from {ago(st.rates.updated, clock.now)} · {st.rates.source === "er-api" ? "ExchangeRate-API" : "jsDelivr"}
      {/if}
    </div>
  {:else}
    <div class="empty muted-text" in:fade>
      {st.error ? "Couldn't load exchange rates. Check your connection." : "Loading exchange rates…"}
    </div>
  {/if}

  {#if picking}
    <div class="picker" transition:fly={{ y: 16, duration: 240, easing: cubicOut }}>
      <div class="search">
        <input bind:this={search} bind:value={query} placeholder="Search currencies" onkeydown={pickKey} />
        <button class="icon-btn small" onclick={() => (picking = null)} aria-label="Close"><Icon name="close" size={14} /></button>
      </div>
      <div class="list">
        {#each matches as c (c.code)}
          {@const current = c.code === (picking === "base" ? base : quote)}
          <button class="opt" class:current onclick={() => choose(c.code)}>
            <span class="ocode">{c.code}</span>
            <span class="oname">{c.name}</span>
            {#if current}<Icon name="check" size={14} />{/if}
          </button>
        {:else}
          <div class="none muted-text">No currency matches “{query}”</div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    overflow: hidden;
  }
  .small {
    width: 26px;
    height: 26px;
  }
  .spinning :global(.icon) {
    animation: spin 700ms var(--ease-out) infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(-360deg);
    }
  }
  .convert {
    display: flex;
    flex-direction: column;
    align-items: stretch;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 48px;
    padding: 0 6px 0 12px;
    border-radius: 12px;
    background: var(--raised);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .field:focus-within {
    border-color: var(--accent);
  }
  .amount,
  .result {
    flex: 1;
    min-width: 0;
    font: 600 20px var(--font-display);
    font-variant-numeric: tabular-nums;
    background: none;
    border: none;
    outline: none;
    color: inherit;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .out {
    background: var(--accent-soft);
    border-color: transparent;
  }
  .code {
    height: 34px;
    padding: 0 10px;
    border-radius: 8px;
    font-weight: 700;
    letter-spacing: 0.3px;
    transition: background-color var(--t-fast) var(--ease-out);
  }
  .code:hover {
    background: var(--hover);
  }
  .swap {
    align-self: center;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    margin: -8px 0;
    z-index: 1;
    border-radius: 50%;
    background: var(--card-tint);
    backdrop-filter: blur(12px);
    border: 1px solid var(--line);
    transition:
      transform 360ms cubic-bezier(0.32, 0.72, 0, 1),
      background-color var(--t-fast) var(--ease-out);
  }
  .swap:hover {
    background: var(--hover);
  }
  .rate {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: 12px;
  }
  .rate span {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .favs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .fav {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 6px 10px;
    border-radius: 10px;
    background: var(--raised);
    border: 1px solid var(--line);
    transition:
      background-color var(--t-fast) var(--ease-out),
      border-color var(--t-fast) var(--ease-out);
  }
  .fav:hover {
    background: var(--hover);
  }
  .fav.active {
    border-color: var(--accent);
  }
  .fcode {
    font-size: 11px;
    font-weight: 700;
    color: var(--text-dim);
  }
  .fval {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .foot {
    margin-top: auto;
    font-size: 11px;
  }
  .warn {
    color: #e8a33d;
  }
  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    text-align: center;
  }
  .picker {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    padding: 12px;
    gap: 8px;
    background: var(--card-tint);
    backdrop-filter: blur(24px) saturate(1.2);
  }
  .search {
    display: flex;
    gap: 6px;
  }
  .search input {
    flex: 1;
    height: 34px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--raised);
    border: 1px solid var(--line);
    color: inherit;
    outline: none;
  }
  .search input:focus {
    border-color: var(--accent);
  }
  .list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 32px;
    padding: 0 8px;
    border-radius: 7px;
    text-align: left;
    transition: background-color var(--t-fast) var(--ease-out);
  }
  .opt:hover {
    background: var(--hover);
  }
  .opt.current {
    color: var(--accent);
  }
  .ocode {
    width: 3.4ch;
    font-weight: 700;
  }
  .oname {
    flex: 1;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .none {
    padding: 16px 8px;
    text-align: center;
  }
</style>
