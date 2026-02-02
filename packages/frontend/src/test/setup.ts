/**
 * Vitest Test Setup
 * 
 * This file runs before each test file and sets up the testing environment.
 * NO MOCKS - all tests use real infrastructure (LocalTerra, Anvil).
 */

import '@testing-library/jest-dom'
import { afterEach } from 'vitest'
import { cleanup } from '@testing-library/react'

// Cleanup after each test
afterEach(() => {
  cleanup()
})

// Browser API polyfills for jsdom (not mocks - just missing APIs)
if (typeof window !== 'undefined') {
  // ResizeObserver polyfill
  if (!window.ResizeObserver) {
    window.ResizeObserver = class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
  }

  // IntersectionObserver polyfill
  if (!window.IntersectionObserver) {
    window.IntersectionObserver = class IntersectionObserver {
      root = null
      rootMargin = ''
      thresholds: number[] = []
      observe() {}
      unobserve() {}
      disconnect() {}
      takeRecords() { return [] }
    } as unknown as typeof IntersectionObserver
  }

  // matchMedia polyfill
  if (!window.matchMedia) {
    window.matchMedia = (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    })
  }
}

// Environment info for integration tests
console.info('[Test Setup] Using real infrastructure - no mocks')
console.info('[Test Setup] Ensure LocalTerra (localhost:1317) and Anvil (localhost:8545) are running for integration tests')
