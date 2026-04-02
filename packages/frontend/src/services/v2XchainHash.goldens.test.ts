/**
 * Fixed V2 vectors: same inputs/expected as `HashLib.t.sol`, Solana `hash.rs`, multichain-rs / CosmWasm goldens.
 */
import { describe, expect, it } from 'vitest'
import {
  chainIdToBytes32,
  computeXchainHashId,
  evmAddressToBytes32,
} from './hashVerification'

describe('V2 xchain hash goldens (EVM / Terra / Solana parity)', () => {
  it('EVM→EVM ERC20 (HashLib / Solana hash.rs evm_vector_evm_to_evm_erc20)', () => {
    const h = computeXchainHashId(
      chainIdToBytes32(1),
      chainIdToBytes32(56),
      evmAddressToBytes32('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'),
      evmAddressToBytes32('0x70997970C51812dc3A010C7d01b50e0d17dc79C8'),
      evmAddressToBytes32('0x5FbDb2315678afecb367f032d93F642f64180aA3'),
      1_000_000_000_000_000_000n,
      42n,
    )
    expect(h).toBe(
      '0x11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54',
    )
  })

  it('EVM→Terra uluna', () => {
    const h = computeXchainHashId(
      chainIdToBytes32(1),
      chainIdToBytes32(2),
      evmAddressToBytes32('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'),
      '0x00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d',
      '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da',
      995_000n,
      1n,
    )
    expect(h).toBe(
      '0x92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453',
    )
  })

  it('EVM→Terra CW20 token field', () => {
    const cw20 =
      '0x00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d' as const
    const h = computeXchainHashId(
      chainIdToBytes32(1),
      chainIdToBytes32(2),
      evmAddressToBytes32('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'),
      cw20,
      cw20,
      1_000_000n,
      5n,
    )
    expect(h).toBe(
      '0x1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b',
    )
  })

  it('Terra→EVM native→ERC20', () => {
    const h = computeXchainHashId(
      chainIdToBytes32(2),
      chainIdToBytes32(1),
      '0x00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d',
      evmAddressToBytes32('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'),
      evmAddressToBytes32('0x5FbDb2315678afecb367f032d93F642f64180aA3'),
      500_000n,
      3n,
    )
    expect(h).toBe(
      '0x076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438',
    )
  })

  it('Terra→EVM CW20→ERC20', () => {
    const h = computeXchainHashId(
      chainIdToBytes32(2),
      chainIdToBytes32(1),
      '0x00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d',
      evmAddressToBytes32('0x70997970C51812dc3A010C7d01b50e0d17dc79C8'),
      evmAddressToBytes32('0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512'),
      2_500_000n,
      7n,
    )
    expect(h).toBe(
      '0xf1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab',
    )
  })

  it('Terra→Solana full 32-byte dest (HashLib test_TransferHash_TerraToSolana_FullPubkeyDest)', () => {
    const h = computeXchainHashId(
      chainIdToBytes32(2),
      chainIdToBytes32(5),
      '0x00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d',
      '0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20',
      '0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd',
      500_000n,
      99n,
    )
    expect(h).toBe(
      '0x5546e5381d73afc31ae405eea765c2c6c6ead75be0ccbf809cd0ad7be7059f71',
    )
  })
})
