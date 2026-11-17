<script lang="ts">
  import Icon from "../../lib/Icon.svelte";
  import { currency } from "../../lib/live.svelte";
  import { store } from "../../lib/settings.svelte";
  import { openCard } from "../../lib/popups";
  import { WIDGETS } from "../../lib/widgets";
  import { money, rate } from "./currency";

  const live = currency();
  const base = $derived(store.s?.currencyBase ?? "USD");
  const quote = $derived(store.s?.currencyQuote ?? "AED");
  const r = $derived(rate(live.v.rates, base, quote));

  let el: HTMLButtonElement;
  function open() {
    const s = WIDGETS.currency.cardSize!;
    openCard("currency", el, s.width, s.height);
  }
</script>

<button
  class="tile glyph"
  bind:this={el}
  onclick={open}
  data-nodrag
  title={r != null ? `1 ${base} = ${money(r)} ${quote}` : "Exchange rates"}
>
  <Icon name="coins" size="var(--glyph)" />
</button>
