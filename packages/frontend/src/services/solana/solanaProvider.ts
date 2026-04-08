/**
 * Injected Solana wallet provider from `window` (Phantom, Solflare, Backpack, etc.).
 * Shared by connect, transaction send, and RPC selection — keep in one module to avoid cycles.
 */

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function getSolanaBrowserProvider(walletName: string): any {
  if (typeof window === "undefined") return null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const win = window as any;
  switch (walletName.toLowerCase()) {
    case "phantom":
      return win.phantom?.solana;
    case "solflare":
      return win.solflare;
    case "backpack":
      return win.backpack;
    default:
      return win.solana;
  }
}
