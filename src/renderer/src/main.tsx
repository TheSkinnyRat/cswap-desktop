import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './styles/app.css'
import { App } from './App'
import { StoreProvider } from './lib/store'
import { ToastProvider } from './lib/toast'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <StoreProvider>
      <ToastProvider>
        <App />
      </ToastProvider>
    </StoreProvider>
  </StrictMode>
)
