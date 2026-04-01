/**
 * Formatting utilities for CL8Y Bridge
 */

import { DECIMALS, NETWORKS, DEFAULT_NETWORK } from './constants';
import { pow10BigInt } from './pow10';
import {
  formatBaseUnitsEnUs,
  formatCompactBigInt,
  tryParseIntegerMicroString,
} from './bigintAmount';

function microToHumanNumber(microAmount: string | number | bigint, decimals: number): number {
  if (typeof microAmount === 'bigint') {
    const divisor = pow10BigInt(decimals);
    const wholePart = microAmount / divisor;
    const fractionalPart = microAmount % divisor;
    const fractionalStr = fractionalPart.toString().padStart(decimals, '0');
    return parseFloat(`${wholePart}.${fractionalStr}`);
  }
  if (typeof microAmount === 'string') {
    return parseFloat(microAmount) / Math.pow(10, decimals);
  }
  return microAmount / Math.pow(10, decimals);
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

  const bi = tryParseIntegerMicroString(microAmount);
  if (bi !== null) {
    return formatBaseUnitsEnUs(bi, decimals, maxDecimals, minDecimals, true);
  }

  const amount = microToHumanNumber(microAmount, decimals);
  return amount.toLocaleString('en-US', {
    minimumFractionDigits: minDecimals,
    maximumFractionDigits: maxDecimals,
  });
}

/** Same rules as formatAmount but no thousands separators (for HTML type="number" inputs). */
export function formatAmountForNumberInput(
  microAmount: string | number | bigint,
  decimals: number = DECIMALS.LUNC,
  displayDecimals?: number
): string {
  const maxDecimals = displayDecimals ?? Math.min(decimals, 6);
  const minDecimals = Math.min(2, maxDecimals);

  const bi = tryParseIntegerMicroString(microAmount);
  if (bi !== null) {
    return formatBaseUnitsEnUs(bi, decimals, maxDecimals, minDecimals, false);
  }

  const amount = microToHumanNumber(microAmount, decimals);
  return amount.toLocaleString('en-US', {
    minimumFractionDigits: minDecimals,
    maximumFractionDigits: maxDecimals,
    useGrouping: false,
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
  if (typeof value === 'bigint') {
    return formatCompactBigInt(value, decimals, sigfigs);
  }
  const bi = tryParseIntegerMicroString(value);
  if (bi !== null) {
    return formatCompactBigInt(bi, decimals, sigfigs);
  }

  let num: number
  if (typeof value === 'string') {
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
    // Show plain decimal instead of scientific notation (e.g. "0.000001" not "1e-6")
    const exp = Math.floor(Math.log10(abs))
    const decimalPlaces = Math.max(-exp + (sigfigs - 1), 1)
    const formatted = num.toFixed(decimalPlaces)
    // Strip trailing zeros but keep at least one decimal place
    return formatted.replace(/(\.\d*?)0+$/, '$1').replace(/\.$/, '')
  }
  return numToSigFig(num, sigfigs)
}

function numToSigFig(n: number, sigfigs: number): string {
  if (n === 0) return '0'
  const s = n.toPrecision(sigfigs)
  return parseFloat(s).toString()
}

/**
 * Expands scientific notation (e.g. "1e+21", "1.5e-3") to a plain decimal string.
 * Used so downstream BigInt() never sees exponential form.
 */
export function expandScientificNotationToDecimalString(sci: string): string {
  const trimmed = sci.trim()
  const m = trimmed.match(/^(-?)(\d+(?:\.\d*)?)[eE]([-+]?\d+)$/)
  if (!m) return trimmed

  const neg = m[1] === '-'
  const coefficient = m[2]!
  const exp = parseInt(m[3]!, 10)
  if (!Number.isFinite(exp)) return '0'

  const coeffParts = coefficient.split('.')
  const intPartRaw = coeffParts[0] ?? ''
  const fracPartRaw = coeffParts[1] ?? ''
  const dotIndex = intPartRaw.length
  const allDigits = intPartRaw + fracPartRaw
  if (allDigits === '' || /^0+$/.test(allDigits)) return '0'

  const sign = neg ? '-' : ''

  if (exp === 0) {
    const t: string = fracPartRaw ? `${intPartRaw}.${fracPartRaw}` : intPartRaw
    return sign + (t.startsWith('.') ? `0${t}` : t)
  }

  if (exp > 0) {
    const newDot = dotIndex + exp
    if (newDot >= allDigits.length) {
      return sign + allDigits + '0'.repeat(newDot - allDigits.length)
    }
    if (newDot <= 0) {
      return sign + `0.${'0'.repeat(-newDot)}${allDigits}`
    }
    return sign + allDigits.slice(0, newDot) + '.' + allDigits.slice(newDot)
  }

  const newDot = dotIndex + exp
  if (newDot <= 0) {
    return sign + `0.${'0'.repeat(-newDot)}${allDigits}`
  }
  return sign + allDigits.slice(0, newDot) + '.' + allDigits.slice(newDot)
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

/** Same as BigInt(parseAmount(...)) without duplicating call sites; relies on parseAmount's safe string output. */
export function parseAmountAsBigInt(
  humanAmount: string | number,
  decimals: number = DECIMALS.LUNC
): bigint {
  return BigInt(parseAmount(humanAmount, decimals))
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

