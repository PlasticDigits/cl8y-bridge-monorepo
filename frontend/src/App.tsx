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