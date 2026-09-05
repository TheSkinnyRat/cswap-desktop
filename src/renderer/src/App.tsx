import { useEffect, useState } from 'react'
import { Sidebar } from './components/Sidebar'
import { TitleBar } from './components/TitleBar'
import { CommandPalette } from './components/CommandPalette'
import { AccountsPage } from './pages/Accounts'
import { AutoPage } from './pages/Auto'
import { MappingsPage } from './pages/Mappings'
import { LogPage } from './pages/Log'
import { SettingsPage } from './pages/Settings'
import { Onboarding } from './pages/Onboarding'
import { useStore, useTheme } from './lib/store'
import { useToast } from './lib/toast'

export function App(): React.JSX.Element {
  const { settings, page, binary, setPage, refresh, updater } = useStore()
  const { notify } = useToast()
  const [palette, setPalette] = useState(false)
  useEffect(() => {
    if (updater.status === 'downloaded') notify('info', `Update ${updater.version} downloaded`, 'Restart from Settings → About to install it; otherwise it installs when you quit.')
  }, [updater.status, updater.version, notify])
  useTheme(settings.theme)

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setPalette((p) => !p)
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'r') {
        e.preventDefault()
        void refresh()
      }
      if ((e.ctrlKey || e.metaKey) && e.key === ',') {
        e.preventDefault()
        setPage('settings')
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [setPage, refresh])

  const needsOnboarding = binary !== null && !binary.version && page !== 'settings' && page !== 'log'

  return (
    <div className="flex h-full flex-col bg-bg">
      <TitleBar onPalette={() => setPalette(true)} />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-y-auto" data-page={page}>
          {needsOnboarding ? (
            <Onboarding />
          ) : page === 'accounts' ? (
            <AccountsPage />
          ) : page === 'auto' ? (
            <AutoPage />
          ) : page === 'mappings' ? (
            <MappingsPage />
          ) : page === 'log' ? (
            <LogPage />
          ) : (
            <SettingsPage />
          )}
        </main>
      </div>
      <CommandPalette open={palette} onClose={() => setPalette(false)} />
    </div>
  )
}
