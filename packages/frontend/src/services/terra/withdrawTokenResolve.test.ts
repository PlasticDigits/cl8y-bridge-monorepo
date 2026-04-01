import { describe, it, expect } from 'vitest'
import { keccak256, toBytes } from 'viem'
import { terraAddressToBytes32 } from '../hashVerification'
import type { TokenlistData } from '../tokenlist'
import {
  resolveTerraWithdrawToken,
  resolveTerraDestTokenIdForRecord,
} from './withdrawTokenResolve'

const miniTokenlist: TokenlistData = {
  name: 'test',
  version: '1',
  tokens: [{ type: 'native', denom: 'uluna', symbol: 'LUNC', name: 'Luna Classic' }],
}

describe('withdrawTokenResolve (glab #89)', () => {
  const cw20Terra = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
  const cw20Bytes32 = terraAddressToBytes32(cw20Terra)

  it('resolveTerraWithdrawToken passes through native / bech32 ids', () => {
    expect(resolveTerraWithdrawToken('uluna', undefined, null)).toBe('uluna')
    expect(resolveTerraWithdrawToken(cw20Terra, undefined, null)).toBe(cw20Terra)
  })

  it('resolveTerraWithdrawToken rejects bare EVM address without destToken bytes32', () => {
    const evmOnly = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
    expect(() => resolveTerraWithdrawToken(evmOnly, undefined, null)).toThrow(
      /Cannot resolve Terra token/,
    )
  })

  it('resolveTerraWithdrawToken maps EVM-style destTokenId + destToken bytes32 → CW20 bech32', () => {
    const evmFallbackId = '0x5FbDB2315678afecb367f032d93F642f64180aa3'
    expect(resolveTerraWithdrawToken(evmFallbackId, cw20Bytes32, null)).toBe(cw20Terra)
  })

  it('resolveTerraWithdrawToken resolves native via tokenlist keccak(denom)', () => {
    const ulunaHash = keccak256(toBytes('uluna'))
    expect(
      resolveTerraWithdrawToken('0x0000000000000000000000000000000000000001', ulunaHash, miniTokenlist),
    ).toBe('uluna')
  })

  it('resolveTerraDestTokenIdForRecord strips EVM id when bytes32 maps to CW20', () => {
    const evmFallbackId = '0x5FbDB2315678afecb367f032d93F642f64180aa3'
    expect(resolveTerraDestTokenIdForRecord(evmFallbackId, cw20Bytes32, null)).toBe(cw20Terra)
  })
})
