import { create } from "zustand";

import type { AccountStatus } from "../../core/generated/AccountStatus";
import { featuresIpc } from "../../core/ipcFeatures";

interface AccountsState {
  status: AccountStatus | null;
  load: () => Promise<void>;
  set: (status: AccountStatus) => void;
}

/** Which accounts are set up (never the secrets), shared by Settings and the account widgets. */
export const useAccounts = create<AccountsState>((set) => ({
  status: null,
  load: async () => set({ status: await featuresIpc.accountStatus() }),
  set: (status) => set({ status }),
}));

/** Load the account status now, and again whenever this window comes back into focus. */
export function watchAccounts(): () => void {
  const reload = () =>
    useAccounts
      .getState()
      .load()
      .catch((error) => console.error("failed to read accounts", error));
  reload();
  window.addEventListener("focus", reload);
  return () => window.removeEventListener("focus", reload);
}
