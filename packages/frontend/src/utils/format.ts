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
 * Format a number in compact human-readable form: configurable sigfigs, k/m/b suffixes, or scientific for small values.
 * Examples: 1234 -> "1.23k", 1234567 -> "1.23m", 0.0000123 -> "1.23e-5"
 */
export function formatCompact(
  value: string | number | bigint,
  decimals: number = 6,
  sigfigs: number = 4
): string {
  let num: number
  if (typeof value === 'bigint') {
    num = Number(value) / Math.pow(10, decimals)
  } else if (typeof value === 'string') {
    num = parseFloat(value) / Math.pow(10, decimals)
  } else {
    num = value / Math.pow(10, decimals)
  }
  if (!Number.isFinite(num) || num === 0) return '0'
  const abs = Math.abs(num)
  if (abs >= 1e9) {
    const scaled = num / 1e9
    return numToSigFig(scaled, sigfigs) + 'b'
  }
  if (abs >= 1e6) {
    const scaled = num / 1e6
    return numToSigFig(scaled, sigfigs) + 'm'
  }
  if (abs >= 1e3) {
    const scaled = num / 1e3
    return numToSigFig(scaled, sigfigs) + 'k'
  }
  if (abs < 0.0001) {
    const exp = Math.floor(Math.log10(abs))
    const mantissa = num / Math.pow(10, exp)
    return `${numToSigFig(mantissa, sigfigs)}e${exp}`
  }
  return numToSigFig(num, sigfigs)
}

function numToSigFig(n: number, sigfigs: number): string {
  if (n === 0) return '0'
  const s = n.toPrecision(sigfigs)
  return parseFloat(s).toString()
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
 * Format a duration in seconds to human-readable.
 * Shows seconds when under 1 minute (e.g. "45s") for countdown timers.
 */
export function formatDuration(seconds: number): string {
  if (seconds < 0) return 'Ended';

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (days > 0) {
    return `${days}d ${hours}h ${minutes}m`;
  }
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m`;
  }
  return `${secs}s`;
}

/**
 * Format cancel window as a range: base + 10% rounded (e.g. 600 → "600-660 seconds").
 */
export function formatCancelWindowRange(cancelWindowSeconds: number): string {
  const max = cancelWindowSeconds + Math.round(cancelWindowSeconds * 0.1)
  return `${cancelWindowSeconds}-${max} seconds`
}

/**
 * Format duration in seconds as mm:ss for countdown timers.
 * Updates visibly every second (e.g. "02:30", "01:05", "00:09").
 */
export function formatCountdownMmSs(seconds: number): string {
  if (seconds < 0) return '0:00';

  const totalSecs = Math.floor(seconds);
  const m = Math.floor(totalSecs / 60);
  const s = totalSecs % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
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

/**
 * Get the explorer URL for a token on a specific chain.
 * @param explorerBaseUrl - Base URL (e.g. https://bscscan.com)
 * @param tokenAddress - Token contract address (ERC20 or CW20)
 * @param chainType - 'evm' uses /token/, 'cosmos' uses /address/
 */
export function getTokenExplorerUrl(
  explorerBaseUrl: string,
  tokenAddress: string,
  chainType: 'evm' | 'cosmos'
): string {
  if (!explorerBaseUrl?.trim() || !tokenAddress?.trim()) return ''
  const base = explorerBaseUrl.replace(/\/$/, '')
  if (chainType === 'evm') return `${base}/token/${tokenAddress}`
  return `${base}/address/${tokenAddress}`
}

