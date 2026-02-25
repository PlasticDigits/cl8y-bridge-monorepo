/**
 * Unit Tests for Formatting Utilities
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import {
  formatAmount,
  formatCompact,
  parseAmount,
  formatRate,
  formatDuration,
  formatCountdownMmSs,
  formatAddress,
  formatPercent,
  formatTimestamp,
  getTerraScannerUrl,
  getEvmScannerUrl,
  getTerraAddressUrl,
  getTerraTxUrl,
  getEvmAddressUrl,
  getEvmTxUrl,
  getTokenExplorerUrl,
} from './format'

describe('formatAmount', () => {
  it('formats micro amounts to human readable with default decimals', () => {
    expect(formatAmount('1000000', 6)).toBe('1.00')
    expect(formatAmount('1500000', 6)).toBe('1.50')
    // Full precision is preserved up to decimals limit
    expect(formatAmount('123456789', 6)).toBe('123.456789')
  })

  it('handles string input', () => {
    expect(formatAmount('1000000')).toBe('1.00')
    expect(formatAmount('500000')).toBe('0.50')
  })

  it('handles number input', () => {
    expect(formatAmount(1000000, 6)).toBe('1.00')
    expect(formatAmount(2500000, 6)).toBe('2.50')
  })

  it('handles bigint input', () => {
    expect(formatAmount(BigInt(1000000), 6)).toBe('1.00')
    expect(formatAmount(BigInt('1000000000000'), 6)).toBe('1,000,000.00')
  })

  it('handles zero', () => {
    expect(formatAmount('0', 6)).toBe('0.00')
    expect(formatAmount(0, 6)).toBe('0.00')
    expect(formatAmount(BigInt(0), 6)).toBe('0.00')
  })

  it('formats large amounts with thousands separator', () => {
    expect(formatAmount('1000000000000', 6)).toBe('1,000,000.00')
    expect(formatAmount('999999999999', 6)).toBe('999,999.999999')
  })

  it('handles different decimal places', () => {
    // 18 decimals (ETH/BNB)
    expect(formatAmount('1000000000000000000', 18)).toBe('1.00')
    expect(formatAmount('1500000000000000000', 18)).toBe('1.50')
  })

  it('respects displayDecimals parameter', () => {
    expect(formatAmount('1234567', 6, 2)).toBe('1.23')
    expect(formatAmount('1234567', 6, 4)).toBe('1.2346')
  })

  it('handles small amounts', () => {
    expect(formatAmount('1', 6)).toBe('0.000001')
    expect(formatAmount('100', 6)).toBe('0.0001')
  })
})

describe('formatCompact', () => {
  it('formats thousands with k suffix', () => {
    // 1,234 human = 1234000000 base (6 decimals)
    expect(formatCompact('1234000000', 6)).toBe('1.234k')
    expect(formatCompact('1000000000', 6)).toBe('1k')
    expect(formatCompact('500000000', 6)).toBe('500')
  })

  it('formats millions with m suffix', () => {
    expect(formatCompact('1234567000000', 6)).toBe('1.235m')
    expect(formatCompact('1000000000000', 6)).toBe('1m')
  })

  it('formats billions with b suffix', () => {
    expect(formatCompact('1234567000000000', 6)).toBe('1.235b')
  })

  it('formats small numbers as plain decimals', () => {
    // 0.000012 human = 12 base (6 decimals); abs < 0.0001
    expect(formatCompact('12', 6)).toBe('0.000012')
    expect(formatCompact('1', 6)).toBe('0.000001')
  })

  it('formats mid-range numbers with sigfigs', () => {
    expect(formatCompact('123456', 6)).toBe('0.1235')
    expect(formatCompact('100', 6)).toBe('0.0001')
  })

  it('handles zero', () => {
    expect(formatCompact('0', 6)).toBe('0')
    expect(formatCompact(0, 6)).toBe('0')
  })

  it('handles bigint', () => {
    expect(formatCompact(BigInt('500000000000000000000'), 18)).toBe('500')
    expect(formatCompact(BigInt('500000000000000000000000'), 18)).toBe('500k')
  })

  it('supports custom significant figures', () => {
    expect(formatCompact('1234567000000', 6, 6)).toBe('1.23457m')
    expect(formatCompact('1234567', 6, 6)).toBe('1.23457')
  })
})

describe('parseAmount', () => {
  it('parses human readable to micro amounts', () => {
    expect(parseAmount('1', 6)).toBe('1000000')
    expect(parseAmount('1.5', 6)).toBe('1500000')
    expect(parseAmount('0.5', 6)).toBe('500000')
  })

  it('handles number input', () => {
    expect(parseAmount(1, 6)).toBe('1000000')
    expect(parseAmount(2.5, 6)).toBe('2500000')
  })

  it('handles zero', () => {
    expect(parseAmount('0', 6)).toBe('0')
    expect(parseAmount(0, 6)).toBe('0')
  })

  it('floors fractional micro units', () => {
    // 1.0000001 LUNC should be 1000000 uluna (fraction truncated)
    expect(parseAmount('1.0000001', 6)).toBe('1000000')
  })

  it('handles different decimal places', () => {
    // 18 decimals (ETH)
    expect(parseAmount('1', 18)).toBe('1000000000000000000')
    expect(parseAmount('0.5', 18)).toBe('500000000000000000')
  })

  it('handles large amounts', () => {
    expect(parseAmount('1000000', 6)).toBe('1000000000000')
  })
})

describe('formatRate', () => {
  it('formats rate with default 4 decimals', () => {
    expect(formatRate(1.2345)).toBe('1.2345')
    expect(formatRate(0.9999)).toBe('0.9999')
  })

  it('handles string input', () => {
    expect(formatRate('1.23456789')).toBe('1.2346')
  })

  it('respects custom decimals', () => {
    expect(formatRate(1.23456789, 2)).toBe('1.23')
    expect(formatRate(1.23456789, 8)).toBe('1.23456789')
  })

  it('pads with zeros if needed', () => {
    expect(formatRate(1.5, 4)).toBe('1.5000')
  })
})

describe('formatDuration', () => {
  it('returns "Ended" for negative values', () => {
    expect(formatDuration(-1)).toBe('Ended')
    expect(formatDuration(-100)).toBe('Ended')
  })

  it('formats seconds when under 1 minute', () => {
    expect(formatDuration(0)).toBe('0s')
    expect(formatDuration(15)).toBe('15s')
    expect(formatDuration(45)).toBe('45s')
  })

  it('formats minutes', () => {
    expect(formatDuration(60)).toBe('1m')
    expect(formatDuration(300)).toBe('5m')
  })

  it('formats hours and minutes', () => {
    expect(formatDuration(3600)).toBe('1h 0m')
    expect(formatDuration(3660)).toBe('1h 1m')
    expect(formatDuration(7200)).toBe('2h 0m')
  })

  it('formats days, hours, and minutes', () => {
    expect(formatDuration(86400)).toBe('1d 0h 0m')
    expect(formatDuration(90000)).toBe('1d 1h 0m')
    expect(formatDuration(172800)).toBe('2d 0h 0m')
  })

  it('handles complex durations', () => {
    // 1 day, 2 hours, 30 minutes = 86400 + 7200 + 1800 = 95400
    expect(formatDuration(95400)).toBe('1d 2h 30m')
  })
})

describe('formatCountdownMmSs', () => {
  it('returns "0:00" for negative values', () => {
    expect(formatCountdownMmSs(-1)).toBe('0:00')
    expect(formatCountdownMmSs(-100)).toBe('0:00')
  })

  it('formats mm:ss with zero-padded seconds', () => {
    expect(formatCountdownMmSs(0)).toBe('0:00')
    expect(formatCountdownMmSs(9)).toBe('0:09')
    expect(formatCountdownMmSs(59)).toBe('0:59')
    expect(formatCountdownMmSs(60)).toBe('1:00')
    expect(formatCountdownMmSs(90)).toBe('1:30')
    expect(formatCountdownMmSs(150)).toBe('2:30')
    expect(formatCountdownMmSs(3661)).toBe('61:01')
  })
})

describe('formatAddress', () => {
  it('truncates long addresses', () => {
    const terraAddr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
    expect(formatAddress(terraAddr, 6)).toBe('terra1...20k38v')
  })

  it('uses default chars value of 8', () => {
    const evmAddr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
    // Default is 8 chars on each side
    expect(formatAddress(evmAddr)).toBe('0xf39Fd6...fFb92266')
  })

  it('returns short addresses unchanged', () => {
    expect(formatAddress('0x1234', 8)).toBe('0x1234')
    expect(formatAddress('test', 4)).toBe('test')
  })

  it('handles edge cases', () => {
    // Exactly at the threshold (chars * 2 + 3)
    expect(formatAddress('12345678901234567890', 8)).toBe('12345678...34567890')
    // Just under threshold
    expect(formatAddress('1234567890123456789', 8)).toBe('1234567890123456789')
  })
})

describe('formatPercent', () => {
  it('formats decimal to percentage', () => {
    expect(formatPercent(0.5)).toBe('50.00%')
    expect(formatPercent(0.25)).toBe('25.00%')
    expect(formatPercent(1)).toBe('100.00%')
  })

  it('respects custom decimals', () => {
    expect(formatPercent(0.12345, 0)).toBe('12%')
    expect(formatPercent(0.12345, 4)).toBe('12.3450%')
  })

  it('handles zero', () => {
    expect(formatPercent(0)).toBe('0.00%')
  })

  it('handles values over 100%', () => {
    expect(formatPercent(1.5)).toBe('150.00%')
  })
})

describe('formatTimestamp', () => {
  // Mock Date to ensure consistent results
  beforeEach(() => {
    vi.useFakeTimers()
  })

  it('formats seconds timestamp', () => {
    const timestamp = 1704067200 // 2024-01-01 00:00:00 UTC
    const result = formatTimestamp(timestamp)
    expect(result).toContain('2024')
  })

  it('handles string input', () => {
    const timestamp = '1704067200'
    const result = formatTimestamp(timestamp)
    expect(result).toContain('2024')
  })

  it('handles nanosecond timestamps', () => {
    // Nanoseconds (> 1e15) should be converted to milliseconds
    const nanoseconds = 1704067200000000000
    const result = formatTimestamp(nanoseconds)
    expect(result).toContain('2024')
  })
})

describe('Scanner URL functions', () => {
  // These depend on the DEFAULT_NETWORK constant
  // In local dev mode, scanners may be empty strings

  it('getTerraScannerUrl returns a string', () => {
    const url = getTerraScannerUrl()
    expect(typeof url).toBe('string')
  })

  it('getEvmScannerUrl returns a string', () => {
    const url = getEvmScannerUrl()
    expect(typeof url).toBe('string')
  })

  it('getTerraAddressUrl formats correctly', () => {
    const url = getTerraAddressUrl('terra1abc123')
    expect(url).toContain('/address/terra1abc123')
  })

  it('getTerraTxUrl formats correctly', () => {
    const url = getTerraTxUrl('ABCD1234')
    expect(url).toContain('/tx/ABCD1234')
  })

  it('getEvmAddressUrl formats correctly', () => {
    const url = getEvmAddressUrl('0x1234')
    expect(url).toContain('/address/0x1234')
  })

  it('getEvmTxUrl formats correctly', () => {
    const url = getEvmTxUrl('0xabc')
    expect(url).toContain('/tx/0xabc')
  })

  it('getTokenExplorerUrl builds EVM token URL', () => {
    expect(getTokenExplorerUrl('https://bscscan.com', '0xabc123', 'evm')).toBe('https://bscscan.com/token/0xabc123')
  })

  it('getTokenExplorerUrl builds Cosmos token URL', () => {
    expect(getTokenExplorerUrl('https://finder.terraclassic.community/mainnet', 'terra1abc', 'cosmos')).toBe(
      'https://finder.terraclassic.community/mainnet/address/terra1abc'
    )
  })

  it('getTokenExplorerUrl returns empty when base or address missing', () => {
    expect(getTokenExplorerUrl('', '0xabc', 'evm')).toBe('')
    expect(getTokenExplorerUrl('https://bscscan.com', '', 'evm')).toBe('')
  })
})
