---
output_files:
  - ../../frontend/src/main.tsx
  - ../../frontend/src/App.tsx
  - ../../frontend/src/index.css
  - ../../frontend/src/vite-env.d.ts
sequential: true
output_dir: ../../frontend/src/
output_file: main.tsx
depends_on:
  - sprint4_008_frontend_config
---

# Frontend Main App Files

Create the main React application entry point and App component.

## src/main.tsx

```tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { WagmiProvider } from 'wagmi'
import { config } from './lib/wagmi'
import App from './App'
import './index.css'

const queryClient = new QueryClient()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <WagmiProvider config={config}>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </WagmiProvider>
  </React.StrictMode>,
)
```

## src/App.tsx

```tsx
import { useState } from 'react'
import { ConnectWallet } from './components/ConnectWallet'
import { BridgeForm } from './components/BridgeForm'
import { TransactionHistory } from './components/TransactionHistory'

function App() {
  const [activeTab, setActiveTab] = useState<'bridge' | 'history'>('bridge')

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900">
      {/* Header */}
      <header className="border-b border-gray-700 bg-gray-900/50 backdrop-blur-sm">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-16">
            <div className="flex items-center gap-2">
              <div className="w-8 h-8 bg-gradient-to-r from-blue-500 to-purple-600 rounded-lg"></div>
              <span className="text-xl font-bold text-white">CL8Y Bridge</span>
            </div>
            <ConnectWallet />
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-2xl mx-auto px-4 py-12">
        {/* Tab Navigation */}
        <div className="flex gap-2 mb-8">
          <button
            onClick={() => setActiveTab('bridge')}
            className={`px-4 py-2 rounded-lg font-medium transition-colors ${
              activeTab === 'bridge'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-800 text-gray-400 hover:text-white'
            }`}
          >
            Bridge
          </button>
          <button
            onClick={() => setActiveTab('history')}
            className={`px-4 py-2 rounded-lg font-medium transition-colors ${
              activeTab === 'history'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-800 text-gray-400 hover:text-white'
            }`}
          >
            History
          </button>
        </div>

        {/* Content */}
        <div className="bg-gray-800/50 backdrop-blur-sm rounded-2xl border border-gray-700 p-6">
          {activeTab === 'bridge' ? <BridgeForm /> : <TransactionHistory />}
        </div>
      </main>

      {/* Footer */}
      <footer className="fixed bottom-0 left-0 right-0 py-4 text-center text-gray-500 text-sm">
        <p>CL8Y Bridge - Cross-chain transfers between Terra Classic and EVM chains</p>
      </footer>
    </div>
  )
}

export default App
```

## src/index.css

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

:root {
  font-family: Inter, system-ui, Avenir, Helvetica, Arial, sans-serif;
  line-height: 1.5;
  font-weight: 400;
  color: rgba(255, 255, 255, 0.87);
  background-color: #1a1a1a;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  margin: 0;
  min-width: 320px;
  min-height: 100vh;
}

/* Custom scrollbar */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: #1a1a1a;
}

::-webkit-scrollbar-thumb {
  background: #4a5568;
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: #718096;
}

/* Animation utilities */
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.animate-spin-slow {
  animation: spin 3s linear infinite;
}

/* Glass effect */
.glass {
  @apply bg-white/5 backdrop-blur-md border border-white/10;
}

/* Gradient text */
.gradient-text {
  @apply bg-gradient-to-r from-blue-400 to-purple-500 bg-clip-text text-transparent;
}
```

## src/vite-env.d.ts

```typescript
/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_EVM_RPC_URL: string
  readonly VITE_TERRA_LCD_URL: string
  readonly VITE_EVM_BRIDGE_ADDRESS: string
  readonly VITE_TERRA_BRIDGE_ADDRESS: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
```
