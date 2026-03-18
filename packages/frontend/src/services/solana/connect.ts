import { Connection, PublicKey } from "@solana/web3.js";

export interface SolanaWalletState {
  connected: boolean;
  publicKey: PublicKey | null;
  address: string | null;
}

/**
 * Connect to a Solana wallet via the browser extension.
 */
export async function connectSolanaWallet(
  walletName: string
): Promise<SolanaWalletState> {
  const provider = getSolanaProvider(walletName);
  if (!provider) {
    throw new Error(`${walletName} wallet not found. Is it installed?`);
  }

  try {
    const resp = await provider.connect();
    const publicKey = resp.publicKey || provider.publicKey;

    if (!publicKey) {
      throw new Error("Failed to get public key from wallet");
    }

    return {
      connected: true,
      publicKey,
      address: publicKey.toBase58(),
    };
  } catch (error: unknown) {
    if (error instanceof Error && 'code' in error && (error as { code: number }).code === 4001) {
      throw new Error("User rejected the connection request");
    }
    throw error;
  }
}

/**
 * Disconnect the Solana wallet.
 */
export async function disconnectSolanaWallet(walletName: string): Promise<void> {
  const provider = getSolanaProvider(walletName);
  if (provider?.disconnect) {
    await provider.disconnect();
  }
}

/**
 * Get the Solana provider from the window object.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function getSolanaProvider(walletName: string): any {
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

/**
 * Create a Solana connection for a given RPC URL.
 */
export function createSolanaConnection(rpcUrl: string): Connection {
  return new Connection(rpcUrl, "confirmed");
}
