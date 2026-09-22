import React from 'react'
import { render, RenderOptions } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { WagmiProvider } from 'wagmi'
import { buildBridgeWagmiConfig, setActiveWagmiConfig } from '../lib/wagmi'
import { writeTestStorageConsentAccept } from '../lib/storageConsent'

const testWagmiConfig = buildBridgeWagmiConfig({})
setActiveWagmiConfig(testWagmiConfig)
if (typeof window !== 'undefined') {
  writeTestStorageConsentAccept()
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
  },
})

interface AllProvidersProps {
  children: React.ReactNode
}

function AllProviders({ children }: AllProvidersProps) {
  return (
    <WagmiProvider config={testWagmiConfig} reconnectOnMount={false}>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>{children}</BrowserRouter>
      </QueryClientProvider>
    </WagmiProvider>
  )
}

export function renderWithProviders(
  ui: React.ReactElement,
  options?: Omit<RenderOptions, 'wrapper'>
) {
  return render(ui, {
    wrapper: AllProviders,
    ...options,
  })
}
