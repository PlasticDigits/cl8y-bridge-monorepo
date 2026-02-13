/**
 * Test account constants for E2E tests.
 * All accounts are deterministic from Anvil/LocalTerra default mnemonics.
 */

// Anvil default accounts (from mnemonic: test test test test test test test test test test test junk)
export const EVM_ACCOUNTS = {
  deployer: {
    address: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
    privateKey: '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
  },
  account1: {
    address: '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
    privateKey: '0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d',
  },
  account2: {
    address: '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
    privateKey: '0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a',
  },
} as const

// LocalTerra test account
// Mnemonic: notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius
export const TERRA_ACCOUNT = {
  address: 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v',
  keyName: 'test1',
} as const

// Chain endpoints
export const ENDPOINTS = {
  anvil: 'http://localhost:8545',
  anvil1: 'http://localhost:8546',
  terraRpc: 'http://localhost:26657',
  terraLcd: 'http://localhost:1317',
} as const
