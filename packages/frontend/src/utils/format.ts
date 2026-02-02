/**
 * Formatting utilities for CL8Y Bridge
 */

import { DECIMALS, NETWORKS, DEFAULT_NETWORK } from './constants';

/**
 * Format a micro-denominated amount to human-readable
 * @param microAmount The amount in micro units (accepts string, number, or bigint)
 * @param decimals The number of decimal places for conversion (default: USTC = 6)
 * @param displayDecimals Optional max decimal places for display (default: same as decimals)
 */
export function formatAmount(
  microAmount: string | number | bigint,
  decimals: number = DECIMALS.LUNC,
  displayDecimals?: number
): string {
  let amount: number;
  
  if (typeof microAmount === 'bigint') {
    // For bigint, convert to string first to preserve precision
    const divisor = BigInt(10 ** decimals);
    const wholePart = microAmount / divisor;
    const fractionalPart = microAmount % divisor;
    const fractionalStr = fractionalPart.toString().padStart(decimals, '0');
    amount = parseFloat(`${wholePart}.${fractionalStr}`);
  } else if (typeof microAmount === 'string') {
    amount = parseFloat(microAmount) / Math.pow(10, decimals);
  } else {
    amount = microAmount / Math.pow(10, decimals);
  }
  
  const maxDecimals = displayDecimals ?? Math.min(decimals, 6);
  const minDecimals = Math.min(2, maxDecimals);
  
  return amount.toLocaleString('en-US', {
    minimumFractionDigits: minDecimals,
    maximumFractionDigits: maxDecimals,
  });
}

/**
 * Parse a human-readable amount to micro-denominated
 */
export function parseAmount(
  humanAmount: string | number,
  decimals: number = DECIMALS.LUNC
): string {
  const amount = typeof humanAmount === 'string' 
    ? parseFloat(humanAmount) 
    : humanAmount;
  
  return Math.floor(amount * Math.pow(10, decimals)).toString();
}

/**
 * Format an exchange rate
 * @param rate The rate value
 * @param decimals Number of decimal places (default 4, use 8 for ticking display)
 */
export function formatRate(rate: string | number, decimals: number = 4): string {
  const rateNum = typeof rate === 'string' ? parseFloat(rate) : rate;
  return rateNum.toFixed(decimals);
}

/**
 * Format a duration in seconds to human-readable
 */
export function formatDuration(seconds: number): string {
  if (seconds < 0) return 'Ended';
  
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  
  if (days > 0) {
    return `${days}d ${hours}h ${minutes}m`;
  }
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

/**
 * Format an address for display (truncated)
 */
export function formatAddress(address: string, chars: number = 8): string {
  if (address.length <= chars * 2 + 3) return address;
  return `${address.slice(0, chars)}...${address.slice(-chars)}`;
}

/**
 * Format a percentage
 */
export function formatPercent(value: number, decimals: number = 2): string {
  return `${(value * 100).toFixed(decimals)}%`;
}

/**
 * Format a timestamp to locale string
 */
export function formatTimestamp(timestamp: number | string): string {
  const ts = typeof timestamp === 'string' ? parseInt(timestamp) : timestamp;
  // Convert nanoseconds to milliseconds if needed
  const ms = ts > 1e15 ? ts / 1e6 : ts * 1000;
  return new Date(ms).toLocaleString();
}

/**
 * Get the Terra scanner base URL for the current network
 */
export function getTerraScannerUrl(): string {
  return NETWORKS[DEFAULT_NETWORK].terra.scanner;
}

/**
 * Get the EVM scanner base URL for the current network
 */
export function getEvmScannerUrl(): string {
  return NETWORKS[DEFAULT_NETWORK].evm.scanner;
}

/**
 * Get the scanner URL for a Terra address
 */
export function getTerraAddressUrl(address: string): string {
  return `${getTerraScannerUrl()}/address/${address}`;
}

/**
 * Get the scanner URL for a Terra transaction hash
 */
export function getTerraTxUrl(txHash: string): string {
  return `${getTerraScannerUrl()}/tx/${txHash}`;
}

/**
 * Get the scanner URL for an EVM address
 */
export function getEvmAddressUrl(address: string): string {
  return `${getEvmScannerUrl()}/address/${address}`;
}

/**
 * Get the scanner URL for an EVM transaction hash
 */
export function getEvmTxUrl(txHash: string): string {
  return `${getEvmScannerUrl()}/tx/${txHash}`;
}

