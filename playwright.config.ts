import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: 'test/e2e',
  timeout: 90_000,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  outputDir: 'test-results',
  use: { trace: 'retain-on-failure' }
})
