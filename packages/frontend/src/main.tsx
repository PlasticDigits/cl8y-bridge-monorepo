import { Buffer } from 'buffer'
window.Buffer = Buffer

import React, { Suspense, lazy } from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { WagmiProvider } from 'wagmi'
import { config } from './lib/wagmi'
import { Layout } from './components/Layout'
import { validateEnv } from './utils/validateEnv'
import { loadChainlist } from './utils/chainlist'
import './index.css'

validateEnv()

const queryClient = new QueryClient()

const TransferPage = lazy(() => import('./pages/TransferPage'))
const TransferStatusPage = lazy(() => import('./pages/TransferStatusPage'))
const HistoryPage = lazy(() => import('./pages/HistoryPage'))
const HashVerificationPage = lazy(() => import('./pages/HashVerificationPage'))
const SettingsPage = lazy(() => import('./pages/SettingsPage'))

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route
            path="/"
            element={
              <Suspense fallback={<PageFallback />}>
                <TransferPage />
              </Suspense>
            }
          />
          <Route
            path="/transfer/:xchainHashId"
            element={
              <Suspense fallback={<PageFallback />}>
                <TransferStatusPage />
              </Suspense>
            }
          />
          <Route
            path="/history"
            element={
              <Suspense fallback={<PageFallback />}>
                <HistoryPage />
              </Suspense>
            }
          />
          <Route
            path="/verify"
            element={
              <Suspense fallback={<PageFallback />}>
                <HashVerificationPage />
              </Suspense>
            }
          />
          <Route
            path="/settings"
            element={
              <Suspense fallback={<PageFallback />}>
                <SettingsPage />
              </Suspense>
            }
          />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}

function PageFallback() {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-24">
      <img
        src="/assets/loading-bridge.png"
        alt=""
        className="h-12 w-12 animate-spin-slow object-contain"
        aria-hidden
      />
      <span className="text-sm text-gray-400">Loading…</span>
    </div>
  )
}

async function init() {
  await loadChainlist()
  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <WagmiProvider config={config}>
        <QueryClientProvider client={queryClient}>
          <App />
        </QueryClientProvider>
      </WagmiProvider>
    </React.StrictMode>,
  )
}
init()
