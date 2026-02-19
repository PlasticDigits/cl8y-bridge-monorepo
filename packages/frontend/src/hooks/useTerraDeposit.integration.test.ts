/**
 * Integration Tests for useTerraDeposit Hook
 *
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  REQUIRES LOCAL INFRASTRUCTURE FOR INTEGRATION TESTS.               │
 * │                                                                    │
 * │  At minimum for integration:                                       │
 * │    1. LocalTerra (Terra Classic devnet) → localhost:1317           │
 * │    2. Terra bridge contract deployed                               │
 * │                                                                    │
 * │  Quick start:                                                      │
 * │    make test-bridge-integration                                    │
 * │                                                                    │
 * │  Or manually:                                                      │
 * │    docker compose up -d anvil anvil1 localterra postgres           │
 * │    npx vitest run --config vitest.config.integration.ts            │
 * │                                                                    │
 * │  Skip integration: SKIP_INTEGRATION=true npm run test:run          │
 * └─────────────────────────────────────────────────────────────────────┘
 */

import { describe, it, expect, beforeAll } from 'vitest'
import {
  encodeDestChainBase64,
  encodeDestAccountBase64,
} from './useTerraDeposit'

const skipIntegration = process.env.SKIP_INTEGRATION === 'true'
const TERRA_LCD_URL = process.env.VITE_TERRA_LCD_URL || 'http://localhost:1317'

describe('useTerraDeposit Integration Tests', () => {
  describe('V2 Encoding Helpers', () => {
    it('should have useTerraDeposit hook structure', async () => {
      const { useTerraDeposit } = await import('./useTerraDeposit')
      expect(useTerraDeposit).toBeTypeOf('function')
    })

    it('should encode chain ID as 4-byte big-endian base64', () => {
      expect(encodeDestChainBase64(31337)).toBe('AAB6aQ==')
      expect(encodeDestChainBase64(31338)).toBe('AAB6ag==')
      expect(encodeDestChainBase64(56)).toBe('AAAAOA==')
      expect(encodeDestChainBase64(204)).toBe('AAAAzA==')
    })

    it('should round-trip encode then decode chain ID', () => {
      const chainId = 31337
      const b64 = encodeDestChainBase64(chainId)
      const decoded = atob(b64)
      expect(decoded.length).toBe(4)
      const bytes = new Uint8Array(decoded.length)
      for (let i = 0; i < decoded.length; i++) {
        bytes[i] = decoded.charCodeAt(i)
      }
      const restored =
        (bytes[0]! << 24) | (bytes[1]! << 16) | (bytes[2]! << 8) | bytes[3]!
      expect(restored).toBe(chainId)
    })

    it('should encode EVM address as left-padded bytes32 base64', () => {
      const evmAddr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      const encoded = encodeDestAccountBase64(evmAddr)
      expect(encoded).toBeTruthy()
      const decoded = atob(encoded)
      expect(decoded.length).toBe(32)
      // First 12 bytes should be zero
      for (let i = 0; i < 12; i++) {
        expect(decoded.charCodeAt(i)).toBe(0)
      }
      // Last 20 bytes = EVM address
      const last20 = decoded.slice(12)
      const hex = Array.from(last20)
        .map((c) => c.charCodeAt(0).toString(16).padStart(2, '0'))
        .join('')
      expect(hex.toLowerCase()).toBe(evmAddr.slice(2).toLowerCase())
    })

    it('should encode Terra address as left-padded bytes32 base64', () => {
      const terraAddr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
      const encoded = encodeDestAccountBase64(terraAddr)
      expect(encoded).toBeTruthy()
      const decoded = atob(encoded)
      expect(decoded.length).toBe(32)
      // First 12 bytes should be zero (20-byte Terra address left-padded)
      for (let i = 0; i < 12; i++) {
        expect(decoded.charCodeAt(i)).toBe(0)
      }
    })

    it('should produce different encodings for different addresses', () => {
      const evm1 = encodeDestAccountBase64('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266')
      const evm2 = encodeDestAccountBase64('0x70997970C51812dc3A010C7d01b50e0d17dc79C8')
      expect(evm1).not.toBe(evm2)

      const terra1 = encodeDestAccountBase64('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')
      const terra2 = encodeDestAccountBase64('terra1dcegyrekltswvyy0xy69ydgxn9x8x32zdtapd8')
      expect(terra1).not.toBe(terra2)
    })
  })

  describe.skipIf(skipIntegration)('LocalTerra Connectivity', () => {
    let lcdUp = false

    beforeAll(async () => {
      try {
        const res = await fetch(`${TERRA_LCD_URL}/cosmos/base/tendermint/v1beta1/node_info`, {
          signal: AbortSignal.timeout(5000),
        })
        lcdUp = res.ok
      } catch {
        lcdUp = false
      }

      if (!lcdUp) {
        throw new Error(
          [
            '',
            '╔══════════════════════════════════════════════════════════════════╗',
            '║  INTEGRATION TEST ABORTED — LocalTerra LCD is not reachable     ║',
            '╠══════════════════════════════════════════════════════════════════╣',
            '║  This test requires LocalTerra at localhost:1317.               ║',
            '║                                                                ║',
            '║  Quick start:  make test-bridge-integration                     ║',
            '║  Or:          docker compose up -d localterra                  ║',
            '║  Then run:    npx vitest run --config vitest.config.integration.ts ║',
            '╚══════════════════════════════════════════════════════════════════╝',
            '',
          ].join('\n')
        )
      }
    })

    it('should reach LocalTerra LCD node_info', async () => {
      const res = await fetch(
        `${TERRA_LCD_URL}/cosmos/base/tendermint/v1beta1/node_info`
      )
      expect(res.ok).toBe(true)
      const data = await res.json()
      expect(data.default_node_info).toBeDefined()
      expect(data.default_node_info.network).toBeDefined()
    })
  })

  it.skip('should execute Terra deposit_native when LocalTerra is running', async () => {
    // Full integration: connect Terra wallet (or use mnemonic), call deposit_native with uluna
    // Requires: LocalTerra, Terra bridge contract, test account with uluna
    // The executeContractWithCoins in terra service requires getConnectedWallet()
    // which needs a browser extension. For headless integration tests we would need
    // to add a test-only path that uses cosmjs with mnemonic to sign and broadcast.
    expect(true).toBe(true)
  })
})
