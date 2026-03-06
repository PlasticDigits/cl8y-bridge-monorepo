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

  // HTMLCanvasElement.getContext polyfill (jsdom returns null; needed by react-blockies)
  const noop = () => {}
  HTMLCanvasElement.prototype.getContext = (() => ({
    fillStyle: '',
    fillRect: noop,
    clearRect: noop,
    getImageData: (_x: number, _y: number, w: number, h: number) => ({ data: new Array(w * h * 4).fill(0) }),
    putImageData: noop,
    createImageData: () => ([]),
    setTransform: noop,
    drawImage: noop,
    save: noop,
    restore: noop,
    beginPath: noop,
    moveTo: noop,
    lineTo: noop,
    closePath: noop,
    stroke: noop,
    translate: noop,
    scale: noop,
    rotate: noop,
    arc: noop,
    fill: noop,
    measureText: () => ({ width: 0 }),
    transform: noop,
    rect: noop,
    clip: noop,
    canvas: { width: 0, height: 0 },
  })) as unknown as typeof HTMLCanvasElement.prototype.getContext

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
