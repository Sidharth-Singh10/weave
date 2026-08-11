type DebouncedFunction<T extends unknown[]> = {
  (...args: T): void;
  /** Run any pending invocation immediately and clear the timer. */
  flush: () => void;
  /** Cancel a pending invocation without running it. */
  cancel: () => void;
};

/** Trailing-edge debounce with `flush`/`cancel`, mirroring Excalidraw's
 * debounce (used for `LocalData._save`). The latest args are replayed on
 * flush so no state is lost when flushing before unload. */
export function debounce<T extends unknown[]>(
  fn: (...args: T) => void,
  ms: number
): DebouncedFunction<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let lastArgs: T | undefined;

  const debounced = (...args: T) => {
    lastArgs = args;
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      if (lastArgs !== undefined) {
        fn(...lastArgs);
        lastArgs = undefined;
      }
    }, ms);
  };

  debounced.flush = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    if (lastArgs !== undefined) {
      fn(...lastArgs);
      lastArgs = undefined;
    }
  };

  debounced.cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    lastArgs = undefined;
  };

  return debounced;
}
