import { useState } from 'react'
import { ChainsPanel, TokensPanel, BridgeConfigPanel } from '../components/settings'

type TabId = 'chains' | 'tokens' | 'bridge'

const TABS: { id: TabId; label: string }[] = [
  { id: 'chains', label: 'Chains' },
  { id: 'tokens', label: 'Tokens' },
  { id: 'bridge', label: 'Bridge Config' },
]

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState<TabId>('chains')

  return (
    <div className="space-y-6">
      <div className="shell-panel-strong">
        <h2 className="text-2xl font-semibold text-white mb-3">System Settings</h2>
        <p className="text-muted text-sm mb-6">
          View registered chains, tokens, and bridge configuration. Read-only.
        </p>

        <div className="flex gap-2 border-b-2 border-white/25 pb-2 mb-6" role="tablist">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`tab-${tab.id}`}
              aria-selected={activeTab === tab.id}
              aria-controls={`tabpanel-${tab.id}`}
              onClick={() => setActiveTab(tab.id)}
              className={`px-4 py-2 text-sm font-medium uppercase tracking-wide border transition-colors ${
                activeTab === tab.id
                  ? 'bg-[#b8ff3d] text-black border-black shadow-[2px_2px_0_#000]'
                  : 'text-slate-300 border-transparent hover:border-white/40 hover:bg-zinc-800/60'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        <div role="tabpanel" id={`tabpanel-${activeTab}`} aria-labelledby={`tab-${activeTab}`}>
          {activeTab === 'chains' && <ChainsPanel />}
          {activeTab === 'tokens' && <TokensPanel />}
          {activeTab === 'bridge' && <BridgeConfigPanel />}
        </div>
      </div>
    </div>
  )
}
