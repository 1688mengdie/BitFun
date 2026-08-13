/**
 * Vitest setup: provide an in-memory `localStorage` for the Node test runtime.
 *
 * Node >= 22 exposes an experimental webstorage `localStorage` global. Without a
 * valid `--localstorage-file` path (the default on Node 25) it is a method-less
 * shell, so code guarding with `typeof localStorage === 'undefined'` (zustand
 * persist, dispatchJobStore, FlowChatStore) treats it as real storage and
 * throws `localStorage.getItem is not a function`. Replace the shell with a
 * working in-memory Storage before any store module loads.
 */
import { enableMapSet } from 'immer';

// Enable React's act() environment for every test. Without this, React logs
// "Warning: The current testing environment is not configured to support
// act(...)" once per act()/render() call (~1100+ lines in CI logs), and ~80
// test files each set the flag manually. Setting it once here removes the
// per-file duplication and silences the warning globally.
(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// groupChatStore state uses Map (rooms/members/messages, contract §2.2).
// immer needs the MapSet plugin enabled explicitly to draft-change Maps.
enableMapSet();

if (
  typeof globalThis.localStorage === 'undefined'
  || typeof globalThis.localStorage.getItem !== 'function'
) {
  const values = new Map<string, string>();
  const memoryStorage: Storage = {
    get length(): number {
      return values.size;
    },
    clear(): void {
      values.clear();
    },
    getItem(key: string): string | null {
      return values.get(key) ?? null;
    },
    key(index: number): string | null {
      return Array.from(values.keys())[index] ?? null;
    },
    removeItem(key: string): void {
      values.delete(key);
    },
    setItem(key: string, value: string): void {
      values.set(key, String(value));
    },
  };
  Object.defineProperty(globalThis, 'localStorage', {
    value: memoryStorage,
    configurable: true,
    writable: true,
  });
}
