/**
 * Terra Classic wallet availability detection
 */

export function isStationInstalled(): boolean {
  return typeof window !== 'undefined' && 'station' in window
}

export function isKeplrInstalled(): boolean {
  return typeof window !== 'undefined' && !!window.keplr
}

export function isLeapInstalled(): boolean {
  return typeof window !== 'undefined' && !!window.leap
}

export function isCosmostationInstalled(): boolean {
  return typeof window !== 'undefined' && !!window.cosmostation
}
