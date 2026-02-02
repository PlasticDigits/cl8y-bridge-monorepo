/**
 * Terra Classic wallet integration using cosmes
 * Supports: Station, Keplr, LUNC Dash, Galaxy Station, Leap, Cosmostation
 * 
 * Adapted for CL8Y Bridge from the ustr-cmm reference implementation.
 */
import {
  ConnectedWallet,
  CosmostationController,
  GalaxyStationController,
  KeplrController,
  LeapController,
  LUNCDashController,
  StationController,
  WalletController,
  WalletName,
  WalletType,
} from '@goblinhunt/cosmes/wallet';
import { MsgExecuteContract } from '@goblinhunt/cosmes/client';
import { CosmosTxV1beta1Fee as Fee } from '@goblinhunt/cosmes/protobufs';
import type { UnsignedTx } from '@goblinhunt/cosmes/wallet';
import { NETWORKS, DEFAULT_NETWORK, WC_PROJECT_ID } from '../utils/constants';

// Terra Classic gas configuration
// Terra Classic LCD doesn't support /cosmos/tx/v1beta1/simulate (returns 501),
// so we use fixed gas limits
const GAS_PRICE_ULUNA = '28.325'; // uluna per gas unit
const BRIDGE_GAS_LIMIT = 500000; // Gas for bridge lock/unlock transactions
const EXECUTE_GAS_LIMIT = 350000; // Gas for execute transactions

const networkConfig = NETWORKS[DEFAULT_NETWORK].terra;
const TERRA_CLASSIC_CHAIN_ID = networkConfig.chainId;
const TERRA_RPC_URL = networkConfig.rpc;

// Gas price for Terra Classic (28.325 uluna per gas unit)
const GAS_PRICE = {
  amount: '28.325',
  denom: 'uluna',
};

// Create wallet controllers
const STATION_CONTROLLER = new StationController();
const KEPLR_CONTROLLER = new KeplrController(WC_PROJECT_ID);
const LUNCDASH_CONTROLLER = new LUNCDashController();
const GALAXY_CONTROLLER = new GalaxyStationController(WC_PROJECT_ID);
const LEAP_CONTROLLER = new LeapController(WC_PROJECT_ID);
const COSMOSTATION_CONTROLLER = new CosmostationController(WC_PROJECT_ID);

const CONTROLLERS: Partial<Record<WalletName, WalletController>> = {
  [WalletName.STATION]: STATION_CONTROLLER,
  [WalletName.KEPLR]: KEPLR_CONTROLLER,
  [WalletName.LUNCDASH]: LUNCDASH_CONTROLLER,
  [WalletName.GALAXYSTATION]: GALAXY_CONTROLLER,
  [WalletName.LEAP]: LEAP_CONTROLLER,
  [WalletName.COSMOSTATION]: COSMOSTATION_CONTROLLER,
};

// Store connected wallets
const connectedWallets: Map<string, ConnectedWallet> = new Map();

// Export wallet types for external use
export { WalletName, WalletType };
export type TerraWalletType = 'station' | 'keplr' | 'luncdash' | 'galaxy' | 'leap' | 'cosmostation';

/**
 * Get chain info for Terra Classic
 */
function getChainInfo() {
  return {
    chainId: TERRA_CLASSIC_CHAIN_ID,
    rpc: TERRA_RPC_URL,
    gasPrice: GAS_PRICE,
  };
}

/**
 * Check if Station wallet is installed
 */
export function isStationInstalled(): boolean {
  return typeof window !== 'undefined' && 'station' in window;
}

/**
 * Check if Keplr wallet is installed
 */
export function isKeplrInstalled(): boolean {
  return typeof window !== 'undefined' && !!window.keplr;
}

/**
 * Check if Leap wallet is installed
 */
export function isLeapInstalled(): boolean {
  return typeof window !== 'undefined' && !!window.leap;
}

/**
 * Check if Cosmostation wallet is installed
 */
export function isCosmostationInstalled(): boolean {
  return typeof window !== 'undefined' && !!window.cosmostation;
}

/**
 * Connect to Terra Classic wallet using cosmes
 */
export async function connectTerraWallet(
  walletName: WalletName = WalletName.STATION,
  walletType: WalletType = WalletType.EXTENSION
): Promise<{ address: string; walletType: TerraWalletType; connectionType: WalletType }> {
  const controller = CONTROLLERS[walletName];
  if (!controller) {
    throw new Error(`Unsupported wallet: ${walletName}`);
  }

  try {
    const chainInfo = getChainInfo();
    console.log(`[Wallet] Connecting ${walletName} (${walletType}) to chain ${chainInfo.chainId}`);
    
    const wallets = await controller.connect(walletType, [chainInfo]);
    
    if (wallets.size === 0) {
      // Handle WalletConnect edge cases
      if (walletType === WalletType.WALLETCONNECT) {
        throw new Error(
          'WalletConnect connection failed. The wallet may be connected but unable to verify. ' +
          'Please try disconnecting and reconnecting.'
        );
      }
      throw new Error('No wallets connected');
    }

    // Get the wallet for Terra Classic chain
    const wallet = wallets.get(TERRA_CLASSIC_CHAIN_ID);
    if (!wallet) {
      throw new Error(`Failed to connect to Terra Classic chain (${TERRA_CLASSIC_CHAIN_ID})`);
    }

    connectedWallets.set(TERRA_CLASSIC_CHAIN_ID, wallet);

    // Map wallet name to wallet type string
    const walletTypeMap: Partial<Record<WalletName, TerraWalletType>> = {
      [WalletName.STATION]: 'station',
      [WalletName.KEPLR]: 'keplr',
      [WalletName.LUNCDASH]: 'luncdash',
      [WalletName.GALAXYSTATION]: 'galaxy',
      [WalletName.LEAP]: 'leap',
      [WalletName.COSMOSTATION]: 'cosmostation',
    };

    return {
      address: wallet.address,
      walletType: walletTypeMap[walletName] || 'station',
      connectionType: walletType,
    };
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    
    // Provide specific error messages
    if (walletName === WalletName.KEPLR) {
      if (errorMessage.includes('not installed') || errorMessage.includes('Keplr')) {
        throw new Error('Keplr wallet is not installed. Please install the Keplr extension.');
      }
    }
    
    if (walletName === WalletName.STATION) {
      if (errorMessage.includes('not installed') || errorMessage.includes('Station')) {
        throw new Error('Station wallet is not installed. Please install the Station extension.');
      }
    }
    
    if (errorMessage.includes('User rejected') || errorMessage.includes('rejected')) {
      throw new Error('Connection rejected by user');
    }
    
    // Get wallet display name
    const displayNames: Partial<Record<WalletName, string>> = {
      [WalletName.STATION]: 'Station',
      [WalletName.KEPLR]: 'Keplr',
      [WalletName.LUNCDASH]: 'LUNC Dash',
      [WalletName.GALAXYSTATION]: 'Galaxy Station',
      [WalletName.LEAP]: 'Leap',
      [WalletName.COSMOSTATION]: 'Cosmostation',
    };
    
    throw new Error(`Failed to connect ${displayNames[walletName] || 'wallet'}: ${errorMessage}`);
  }
}

/**
 * Disconnect wallet
 */
export async function disconnectTerraWallet(): Promise<void> {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID);
  if (wallet) {
    const controller = CONTROLLERS[wallet.id];
    if (controller) {
      controller.disconnect([TERRA_CLASSIC_CHAIN_ID]);
    }
    connectedWallets.delete(TERRA_CLASSIC_CHAIN_ID);
  }
}

/**
 * Get current connected wallet
 */
export function getConnectedWallet(): ConnectedWallet | null {
  return connectedWallets.get(TERRA_CLASSIC_CHAIN_ID) || null;
}

/**
 * Get current connected address
 */
export function getCurrentTerraAddress(): string | null {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID);
  return wallet ? wallet.address : null;
}

/**
 * Check if wallet is connected
 */
export function isTerraWalletConnected(): boolean {
  return connectedWallets.has(TERRA_CLASSIC_CHAIN_ID);
}

/**
 * Estimate fee for Terra Classic transaction
 * Terra Classic LCD doesn't support simulation endpoint, so we use fixed gas limits
 */
function estimateTerraClassicFee(gasLimit: number): Fee {
  const feeAmount = Math.ceil(parseFloat(GAS_PRICE_ULUNA) * gasLimit);
  
  return new Fee({
    amount: [
      {
        amount: feeAmount.toString(),
        denom: 'uluna',
      },
    ],
    gasLimit: BigInt(gasLimit),
  });
}

/**
 * Check if error is a sequence mismatch error that can be retried
 */
function isSequenceMismatchError(error: unknown): boolean {
  if (error instanceof Error) {
    const msg = error.message.toLowerCase();
    return (
      msg.includes('sequence') ||
      msg.includes('account sequence mismatch') ||
      msg.includes('signature verification failed')
    );
  }
  return false;
}

/**
 * Reconnect wallet to refresh account info (account number and sequence)
 * This is needed when sequence gets out of sync with the chain
 */
async function reconnectWalletForRefresh(): Promise<void> {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID);
  if (!wallet) {
    throw new Error('No wallet connected');
  }

  const walletId = wallet.id;
  const controller = CONTROLLERS[walletId];
  
  if (!controller) {
    console.warn('Cannot refresh: controller not found for wallet', walletId);
    return;
  }

  console.log('🔄 Refreshing wallet connection to update account info...');
  
  // Disconnect and reconnect to refresh cached account info
  try {
    controller.disconnect([TERRA_CLASSIC_CHAIN_ID]);
    connectedWallets.delete(TERRA_CLASSIC_CHAIN_ID);
    
    // Small delay to ensure cleanup
    await new Promise(resolve => setTimeout(resolve, 500));
    
    // Reconnect
    const chainInfo = getChainInfo();
    const wallets = await controller.connect(WalletType.EXTENSION, [chainInfo]);
    
    const newWallet = wallets.get(TERRA_CLASSIC_CHAIN_ID);
    if (newWallet) {
      connectedWallets.set(TERRA_CLASSIC_CHAIN_ID, newWallet);
      console.log('✅ Wallet reconnected, account info refreshed');
    } else {
      throw new Error('Failed to reconnect wallet');
    }
  } catch (error) {
    console.error('Failed to refresh wallet connection:', error);
    throw new Error('Failed to refresh wallet. Please disconnect and reconnect manually.');
  }
}

/**
 * Execute a contract transaction with native coins (matching preregister pattern)
 * This follows the exact same pattern as the working preregister dapp
 * Includes retry logic for sequence mismatch errors
 */
export async function executeContractWithCoins(
  contractAddress: string,
  executeMsg: Record<string, unknown>,
  coins?: Array<{ denom: string; amount: string }>,
  maxRetries: number = 2
): Promise<{ txHash: string }> {
  let lastError: Error | null = null;
  
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    // Use getConnectedWallet() matching the working preregister pattern
    const wallet = getConnectedWallet();
    if (!wallet) {
      throw new Error('Wallet not connected. Please connect your wallet first.');
    }

    try {
      // Create the MsgExecuteContract message (matching preregister exactly)
      const msg = new MsgExecuteContract({
        sender: wallet.address,
        contract: contractAddress,
        msg: executeMsg,
        funds: coins && coins.length > 0 ? coins : [],
      });

      // Log the raw message for debugging
      console.group(`📝 Raw Transaction Message (attempt ${attempt + 1}/${maxRetries + 1})`);
      console.log('MsgExecuteContract:', {
        sender: wallet.address,
        contract: contractAddress,
        msg: JSON.stringify(executeMsg),
        funds: coins && coins.length > 0 ? coins : [],
      });
      console.groupEnd();

      // Create unsigned transaction
      const unsignedTx: UnsignedTx = {
        msgs: [msg],
        memo: '',
      };

      // Estimate fee using fixed gas limits (Terra Classic doesn't support simulation)
      const fee = estimateTerraClassicFee(BRIDGE_GAS_LIMIT);
      
      console.log('⛽ Fee estimate:', {
        gasLimit: BRIDGE_GAS_LIMIT,
        feeAmount: fee.amount,
      });

      // Broadcast transaction
      const txHash = await wallet.broadcastTx(unsignedTx, fee);
      
      console.log('📡 Transaction broadcast, hash:', txHash);

      // Poll for transaction confirmation
      const { txResponse } = await wallet.pollTx(txHash);

      // Check if transaction failed
      if (txResponse.code !== 0) {
        const errorMsg = 
          txResponse.rawLog ||
          `Transaction failed with code ${txResponse.code}`;
        console.error('❌ Transaction failed:', errorMsg);
        throw new Error(`Transaction failed: ${errorMsg}`);
      }

      console.log('✅ Transaction confirmed successfully');
      return { txHash };
    } catch (error) {
      console.error(`Transaction attempt ${attempt + 1} failed:`, error);
      lastError = error instanceof Error ? error : new Error(String(error));
      
      // Check if this is a sequence mismatch error and we have retries left
      if (isSequenceMismatchError(error) && attempt < maxRetries) {
        console.log(`🔄 Sequence mismatch detected, refreshing wallet and retrying (${maxRetries - attempt} retries left)...`);
        
        try {
          await reconnectWalletForRefresh();
          // Add a small delay before retry
          await new Promise(resolve => setTimeout(resolve, 1000));
          continue; // Retry the transaction
        } catch (refreshError) {
          console.error('Failed to refresh wallet:', refreshError);
          // If refresh fails, throw the original error
          throw handleSwapTransactionError(error);
        }
      }
      
      // Not a sequence error or no retries left
      throw handleSwapTransactionError(error);
    }
  }
  
  // Should not reach here, but just in case
  throw lastError || new Error('Transaction failed after retries');
}

/**
 * Handle transaction errors with user-friendly messages
 */
function handleSwapTransactionError(error: unknown): Error {
  if (error instanceof Error) {
    const errorMessage = error.message;

    // User rejection errors
    if (
      errorMessage.includes('User rejected') ||
      errorMessage.includes('rejected') ||
      errorMessage.includes('User denied') ||
      errorMessage.includes('user rejected')
    ) {
      return new Error('Transaction rejected by user');
    }

    // Sequence mismatch errors (after retries exhausted)
    if (
      errorMessage.includes('sequence') ||
      errorMessage.includes('account sequence mismatch') ||
      errorMessage.includes('signature verification failed')
    ) {
      return new Error(
        'Transaction sequence mismatch. This usually happens when transactions are sent too quickly. ' +
        'Please wait a few seconds and try again, or disconnect and reconnect your wallet.'
      );
    }

    // Network/connection errors
    if (
      errorMessage.includes('Failed to fetch') ||
      errorMessage.includes('NetworkError') ||
      errorMessage.includes('network')
    ) {
      return new Error(
        `Network error: ${errorMessage}. Please check your internet connection and try again.`
      );
    }

    // Include full error message for debugging
    return new Error(`Transaction failed: ${errorMessage}`);
  }

  return new Error(`Transaction failed: ${String(error)}`);
}

/**
 * Execute a CW20 Send message (for referral registration)
 */
export async function executeCw20Send(
  tokenAddress: string,
  recipientContract: string,
  amount: string,
  embeddedMsg: object
): Promise<{ txHash: string }> {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID);
  if (!wallet) {
    throw new Error('Wallet not connected');
  }

  // CW20 Send message - the msg field needs to be base64 encoded
  const sendMsg = {
    send: {
      contract: recipientContract,
      amount: amount,
      msg: btoa(JSON.stringify(embeddedMsg)), // base64 encode the embedded message
    },
  };

  try {
    // Create the MsgExecuteContract message
    const msg = new MsgExecuteContract({
      sender: wallet.address,
      contract: tokenAddress,
      msg: sendMsg,
      funds: [],
    });

    // Create unsigned transaction
    const unsignedTx: UnsignedTx = {
      msgs: [msg],
      memo: '',
    };

    // Estimate fee using fixed gas limits (Terra Classic doesn't support simulation)
    const fee = estimateTerraClassicFee(EXECUTE_GAS_LIMIT);

    // Broadcast transaction
    const txHash = await wallet.broadcastTx(unsignedTx, fee);

    // Poll for transaction confirmation
    const { txResponse } = await wallet.pollTx(txHash);

    // Check if transaction failed
    if (txResponse.code !== 0) {
      const errorMsg = txResponse.rawLog || `Transaction failed with code ${txResponse.code}`;
      throw new Error(errorMsg);
    }

    return { txHash };
  } catch (error) {
    console.error('Transaction execution failed:', error);
    throw error;
  }
}

// Extend window types for wallet detection
declare global {
  interface Window {
    station?: {
      connect: () => Promise<void>;
      disconnect: () => Promise<void>;
    };
    keplr?: {
      enable: (chainId: string) => Promise<void>;
      getOfflineSigner: (chainId: string) => unknown;
    };
    leap?: {
      enable: (chainId: string) => Promise<void>;
      getOfflineSigner: (chainId: string) => unknown;
    };
    cosmostation?: {
      providers: {
        keplr: unknown;
      };
    };
  }
}
