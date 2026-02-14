import { useState } from 'react'
import { ChainsPanel, TokensPanel, BridgeConfigPanel } from '../components/settings'
import { sounds } from '../lib/sounds'

type TabId = 'chains' | 'tokens' | 'bridge'

const TABS: { id: TabId; label: string }[] = [
  { id: 'chains', label: 'Chains' },
  { id: 'tokens', label: 'Tokens' },
  { id: 'bridge', label: 'Bridge Config' },
]

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState<TabId>('chains')

  return (
    <div className="mx-auto max-w-5xl space-y-4">
      <div className="shell-panel-strong relative overflow-hidden">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-8 top-2 h-28 rounded-[24px] theme-hero-glow blur-2xl"
        />
        <div className="relative z-10">
        <h2 className="mb-2 text-xl font-semibold text-white">System Settings</h2>
        <p className="mb-4 text-xs uppercase tracking-wide text-gray-300">
          View registered chains, tokens, and bridge configuration. Read-only.
        </p>

        <div className="mb-4 flex flex-wrap gap-2 border border-white/20 bg-black/35 p-2" role="tablist">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`tab-${tab.id}`}
              aria-selected={activeTab === tab.id}
              aria-controls={`tabpanel-${tab.id}`}
              onClick={() => {
                sounds.playButtonPress()
                setActiveTab(tab.id)
              }}
              className={`px-4 py-2 text-sm font-medium uppercase tracking-wide border transition-colors ${
                activeTab === tab.id
                  ? 'bg-[#202614] text-[#d5ff7f] border-[#b8ff3d]/60 shadow-[2px_2px_0_#000]'
                  : 'text-slate-300 border-white/20 bg-[#161616] hover:border-white/35 hover:text-white'
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
    </div>
  )
}
