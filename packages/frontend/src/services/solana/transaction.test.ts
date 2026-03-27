import { PublicKey } from '@solana/web3.js'
import { describe, expect, it } from 'vitest'
import { bytes4HexToUint8Array, parseTokenMappingLocalMint, WSOL_MINT } from './transaction'

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
})
