/**
 * Formatting utilities for CL8Y Bridge
 */

import { DECIMALS, NETWORKS, DEFAULT_NETWORK } from './constants';
import { pow10BigInt } from './pow10';
import {
  bigintFromBaseUnitsString,
  expandScientificNotationToDecimalString,
} from './scientificDecimal';
export { expandScientificNotationToDecimalString };
import {
  formatCompactHumanRational,
  formatRationalHumanEnUs,
  microRationalToHumanDenominator,
  tryParseMicroRational,
} from './bigintAmount';

/** Em dash: display when a micro amount cannot be parsed as a rational base-unit value. */
export const UNPARSEABLE_AMOUNT_DISPLAY = '\u2014'

/** Empty string: safe for `type="number"` when max/amount cannot be formatted. */
export const UNPARSEABLE_AMOUNT_NUMBER_INPUT = ''

/** Same as {@link UNPARSEABLE_AMOUNT_DISPLAY} for compact labels. */
export const UNPARSEABLE_AMOUNT_COMPACT = UNPARSEABLE_AMOUNT_DISPLAY

function warnUnparseableMicroAmount(
  fn: string,
  microAmount: unknown,
  decimals: number,
  extra?: Record<string, unknown>
): void {
  console.warn(
    `[cl8y-bridge/format] ${fn}: cannot parse micro amount as rational; returning sentinel.`,
    { microAmount, decimals, ...extra }
  )
}

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
  const maxDecimals = displayDecimals ?? Math.min(decimals, 6);
  const minDecimals = Math.min(2, maxDecimals);

  const rat = tryParseMicroRational(microAmount);
  if (rat !== null) {
    const hn = rat.neg ? -rat.n : rat.n;
    const hd = microRationalToHumanDenominator(rat, decimals);
    return formatRationalHumanEnUs(hn, hd, maxDecimals, minDecimals, true);
  }

  warnUnparseableMicroAmount('formatAmount', microAmount, decimals, { displayDecimals });
  return UNPARSEABLE_AMOUNT_DISPLAY;
}

/** Same rules as formatAmount but no thousands separators (for HTML type="number" inputs). */
export function formatAmountForNumberInput(
  microAmount: string | number | bigint,
  decimals: number = DECIMALS.LUNC,
  displayDecimals?: number
): string {
  const maxDecimals = displayDecimals ?? Math.min(decimals, 6);
  const minDecimals = Math.min(2, maxDecimals);

  const rat = tryParseMicroRational(microAmount);
  if (rat !== null) {
    const hn = rat.neg ? -rat.n : rat.n;
    const hd = microRationalToHumanDenominator(rat, decimals);
    return formatRationalHumanEnUs(hn, hd, maxDecimals, minDecimals, false);
  }

  warnUnparseableMicroAmount('formatAmountForNumberInput', microAmount, decimals, { displayDecimals });
  return UNPARSEABLE_AMOUNT_NUMBER_INPUT;
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
  const rat = tryParseMicroRational(value);
  if (rat !== null) {
    const hn = rat.neg ? -rat.n : rat.n;
    const hd = microRationalToHumanDenominator(rat, decimals);
    return formatCompactHumanRational(hn, hd, sigfigs);
  }

  warnUnparseableMicroAmount('formatCompact', value, decimals, { sigfigs });
  return UNPARSEABLE_AMOUNT_COMPACT;
}

function floorHumanDecimalToBaseUnitsString(unsignedDecimal: string, decimals: number): string {
  let u = unsignedDecimal.trim()
  if (u === '' || u === '.') return '0'
  if (/[eE]/.test(u)) u = expandScientificNotationToDecimalString(u)

  const match = u.match(/^(\d*)(?:\.(\d*))?$/)
  if (!match) {
    const fallback = parseFloat(u)
    if (!Number.isFinite(fallback)) return '0'
    return floorHumanDecimalToBaseUnitsString(expandScientificNotationToDecimalString(fallback.toString()), decimals)
  }

  let intPart = match[1] || '0'
  const fracPart = match[2] || ''
  intPart = intPart.replace(/^0+/, '') || '0'

  const fracFloored = (fracPart + '0'.repeat(decimals)).slice(0, decimals).padEnd(decimals, '0')
  return (BigInt(intPart) * pow10BigInt(decimals) + BigInt(fracFloored || '0')).toString()
}

/**
 * Parse a human-readable amount to base units as a decimal string (BigInt-safe, no scientific notation).
 */
export function parseAmount(humanAmount: string | number, decimals: number = DECIMALS.LUNC): string {
  let s: string
  if (typeof humanAmount === 'number') {
    if (!Number.isFinite(humanAmount)) return '0'
    if (humanAmount === 0) return '0'
    s = humanAmount.toString()
  } else {
    s = humanAmount.trim()
  }

  if (s === '' || s === '+' || s === '-') return '0'

  const neg = s.startsWith('-')
  let body = (neg ? s.slice(1) : s).trim()
  if (body.startsWith('+')) body = body.slice(1).trim()
  if (body === '' || body === '.') return '0'

  const raw = floorHumanDecimalToBaseUnitsString(body, decimals)
  if (raw === '0') return '0'
  return neg ? `-${raw}` : raw
}

/** Base units as bigint; uses {@link bigintFromBaseUnitsString} so JSON/scientific forms never throw (GitLab #95). */
export function parseAmountAsBigInt(
  humanAmount: string | number,
  decimals: number = DECIMALS.LUNC
): bigint {
  return bigintFromBaseUnitsString(parseAmount(humanAmount, decimals))
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
  chainType: 'evm' | 'cosmos' | 'solana'
): string {
  if (!explorerBaseUrl?.trim() || !tokenAddress?.trim()) return ''
  const base = explorerBaseUrl.replace(/\/$/, '')
  if (chainType === 'evm') return `${base}/token/${tokenAddress}`
  return `${base}/address/${tokenAddress}`
}

