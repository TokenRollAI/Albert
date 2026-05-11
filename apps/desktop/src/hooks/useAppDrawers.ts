import { useCallback, useMemo, useState } from "react";

/**
 * State for a single drawer/overlay in App.tsx. Each slot exposes a
 * minimal shape: the current open flag plus imperative open/close/toggle
 * actions. Keeping them behind one hook lets the root component stop
 * managing six independent `useState<boolean>` slots.
 */
export interface DrawerSlot {
  readonly open: boolean;
  open$: () => void;
  close: () => void;
  toggle: () => void;
  set: (next: boolean) => void;
}

export interface AppDrawers {
  workspace: DrawerSlot;
  importReport: DrawerSlot;
  import: DrawerSlot;
  mockServer: DrawerSlot;
  providers: DrawerSlot;
  shortcuts: DrawerSlot;
  palette: DrawerSlot;
}

function useDrawerSlot(initial = false): DrawerSlot {
  const [open, set] = useState(initial);
  const open$ = useCallback(() => set(true), []);
  const close = useCallback(() => set(false), []);
  const toggle = useCallback(() => set((prev) => !prev), []);
  return useMemo(
    () => ({ open, open$, close, toggle, set }),
    [open, open$, close, toggle]
  );
}

/**
 * Bundled drawer state for the App root. Each slot is independent — the
 * hook never coordinates them (e.g. it won't auto-close one when another
 * opens) because user muscle memory often wants multiple overlays.
 */
export function useAppDrawers(): AppDrawers {
  return {
    workspace: useDrawerSlot(),
    importReport: useDrawerSlot(),
    import: useDrawerSlot(),
    mockServer: useDrawerSlot(),
    providers: useDrawerSlot(),
    shortcuts: useDrawerSlot(),
    palette: useDrawerSlot()
  };
}
