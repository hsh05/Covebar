// Mouse-wheel adjustment for level tiles (volume, brightness). The tile shows the value it
// just asked for until the backend catches up, so fast scrolling never jumps back.

export function wheelLevel(read: () => number | null, write: (v: number) => void, step: number) {
  const st = $state<{ pending: number | null }>({ pending: null });
  let timer: ReturnType<typeof setTimeout> | undefined;
  return {
    get value() {
      return st.pending ?? read();
    },
    wheel(e: WheelEvent) {
      const cur = st.pending ?? read();
      if (cur == null || e.deltaY === 0) return;
      const dir = e.deltaY < 0 ? 1 : -1;
      const next = Math.max(0, Math.min(100, Math.round(cur / step) * step + dir * step));
      st.pending = next;
      write(next);
      clearTimeout(timer);
      timer = setTimeout(() => (st.pending = null), 900);
    },
  };
}
