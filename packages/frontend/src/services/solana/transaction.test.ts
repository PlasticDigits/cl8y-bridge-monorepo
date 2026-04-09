import { PublicKey } from '@solana/web3.js'
import { describe, expect, it } from 'vitest'
import {
  buildWithdrawSubmitInstruction,
  bytes4HexToUint8Array,
  formatSolanaUserFacingError,
  formatSolanaWalletError,
  looksLikeSolanaLocalnetRpc,
  parseTokenMappingLocalMint,
  WSOL_MINT,
} from './transaction'

describe('solana/transaction helpers', () => {
  it('bytes4HexToUint8Array decodes V2 chain id hex', () => {
    expect([...bytes4HexToUint8Array('0x00000001')]).toEqual([0, 0, 0, 1])
    expect([...bytes4HexToUint8Array('0x00000005')]).toEqual([0, 0, 0, 5])
  })

  it('parseTokenMappingLocalMint reads first field after discriminator', () => {
    const disc = Buffer.alloc(8, 1)
    const mint = new PublicKey(WSOL_MINT)
    const buf = Buffer.concat([disc, mint.toBuffer()])
    expect(parseTokenMappingLocalMint(buf).equals(mint)).toBe(true)
  })

  it('looksLikeSolanaLocalnetRpc detects loopback hosts', () => {
    expect(looksLikeSolanaLocalnetRpc('http://localhost:8899')).toBe(true)
    expect(looksLikeSolanaLocalnetRpc('http://127.0.0.1:8899')).toBe(true)
    expect(looksLikeSolanaLocalnetRpc('https://api.devnet.solana.com')).toBe(false)
  })

  it('formatSolanaWalletError maps rejection codes and objects', () => {
    expect(formatSolanaWalletError(new Error('User rejected'))).toBe('User rejected')
    expect(formatSolanaWalletError({ code: 4001 })).toContain('rejected')
    expect(formatSolanaWalletError({ message: 'not supported on localnet' })).toContain('Phantom often')
  })

  it('formatSolanaUserFacingError uses public-RPC notice on HTTP 403', () => {
    expect(formatSolanaUserFacingError(new Error('403 Forbidden'))).toContain('403')
    expect(formatSolanaUserFacingError(new Error('403 Forbidden'))).toContain('custom RPC')
  })

  it('buildWithdrawSubmitInstruction rejects srcAccount that is not exactly 32 bytes', () => {
    const programId = new PublicKey('11111111111111111111111111111112')
    const payer = new PublicKey('11111111111111111111111111111112')
    const dest = new PublicKey('11111111111111111111111111111112')
    const mint = new PublicKey('11111111111111111111111111111112')
    const srcChain = new Uint8Array([0, 0, 0, 1])
    const srcToken = new Uint8Array(32)
    const bridgeChain = new Uint8Array([0, 0, 0, 5])
    const shortSrc = new Uint8Array(20)

    expect(() =>
      buildWithdrawSubmitInstruction(
        programId,
        payer,
        dest,
        srcChain,
        shortSrc,
        srcToken,
        mint,
        1n,
        1n,
        bridgeChain,
        0n,
      ),
    ).toThrow(/withdraw_submit srcAccount must be 32 bytes/)
  })
})
