/**
 * Ensures V2 transfer-id computation stays centralized in hashVerification (INV-HFE1)
 * and Solana client delegates to it.
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import {
  computeXchainHashId,
  computeXchainHashIdBytes,
  computeXchainHashIdFromDeposit,
  computeXchainHashIdFromWithdraw,
  chainIdToBytes32,
  evmAddressToBytes32,
} from './hashVerification'
import { computeTransferHash } from './solana/transaction'

const __dirname = dirname(fileURLToPath(import.meta.url))
const SRC_ROOT = join(__dirname, '..')

/** Fuzz file builds manual 224-byte preimage on purpose (must not be production). */
const SKIP_NAMES = new Set(['crossChainHash.fuzz.test.ts'])

function walkTsFiles(dir: string, relative: string, out: string[]): void {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name)
    const rel = join(relative, name)
    if (statSync(full).isDirectory()) {
      if (name === 'test' || name === 'e2e') continue
      walkTsFiles(full, rel, out)
      continue
    }
    if (!/\.tsx?$/.test(name)) continue
    if (name.endsWith('.test.ts') || name.endsWith('.test.tsx')) continue
    if (SKIP_NAMES.has(name)) continue
    out.push(rel)
  }
}

describe('hash canonical routing (frontend)', () => {
  it('computeTransferHash matches computeXchainHashIdBytes byte-for-byte', () => {
    const srcChain = new Uint8Array([0, 0, 0, 9])
    const destChain = new Uint8Array([0, 0, 0, 5])
    const srcAccount = new Uint8Array(32)
    srcAccount[31] = 0x11
    const destAccount = new Uint8Array(32)
    destAccount[31] = 0x22
    const token = new Uint8Array(32)
    token[31] = 0x33
    const amount = 12345n
    const nonce = 77n

    const a = computeXchainHashIdBytes(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce,
    )
    const b = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce,
    )
    expect(Buffer.from(a).toString('hex')).toBe(Buffer.from(b).toString('hex'))
  })

  it('only hashVerification.ts uses viem encodeAbiParameters / parseAbiParameters for V2 hash', () => {
    const files: string[] = []
    walkTsFiles(SRC_ROOT, '', files)
    const offenders: string[] = []
    for (const rel of files) {
      const text = readFileSync(join(SRC_ROOT, rel), 'utf8')
      if (text.includes('encodeAbiParameters') || text.includes('parseAbiParameters')) {
        offenders.push(rel)
      }
    }
    expect(offenders.sort()).toEqual(['services/hashVerification.ts'])
  })

  it('computeXchainHashIdFromDeposit / FromWithdraw delegate to computeXchainHashId', () => {
    const srcChain = chainIdToBytes32(1)
    const destChain = chainIdToBytes32(2)
    const srcAccount = evmAddressToBytes32(
      '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
    )
    const destAccount = evmAddressToBytes32(
      '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
    )
    const token = evmAddressToBytes32('0x5FbDb2315678afecb367f032d93F642f64180aA3')
    const amount = 1_000_000n
    const nonce = 3n
    const base = computeXchainHashId(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce,
    )
    const dep = computeXchainHashIdFromDeposit(
      1,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce,
    )
    const wit = computeXchainHashIdFromWithdraw(
      srcChain,
      2,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce,
    )
    expect(dep).toBe(base)
    expect(wit).toBe(base)
  })
})
