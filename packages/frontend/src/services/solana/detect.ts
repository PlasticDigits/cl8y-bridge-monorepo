export interface SolanaWalletInfo {
  name: string;
  icon: string;
  installed: boolean;
  adapter: string;
}

const KNOWN_WALLETS: { name: string; icon: string; windowKey: string; adapter: string }[] = [
  { name: "Phantom", icon: "phantom", windowKey: "phantom", adapter: "PhantomWalletAdapter" },
  { name: "Solflare", icon: "solflare", windowKey: "solflare", adapter: "SolflareWalletAdapter" },
  { name: "Backpack", icon: "backpack", windowKey: "backpack", adapter: "BackpackWalletAdapter" },
  { name: "Coinbase Wallet", icon: "coinbase", windowKey: "coinbaseSolana", adapter: "CoinbaseWalletAdapter" },
];

export function detectSolanaWallets(): SolanaWalletInfo[] {
  if (typeof window === "undefined") {
    return KNOWN_WALLETS.map((w) => ({ ...w, installed: false }));
  }

  return KNOWN_WALLETS.map((wallet) => ({
    name: wallet.name,
    icon: wallet.icon,
    adapter: wallet.adapter,
    installed: (() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const win = window as any;
      const obj = win[wallet.windowKey];
      if (!obj) return false;
      if (wallet.windowKey === "phantom") return !!obj?.solana?.isPhantom;
      if (wallet.windowKey === "solflare") return !!obj?.isSolflare;
      if (wallet.windowKey === "backpack") return !!obj?.isBackpack;
      return !!obj;
    })(),
  }));
}

export function getInstalledSolanaWallets(): SolanaWalletInfo[] {
  return detectSolanaWallets().filter((w) => w.installed);
}
