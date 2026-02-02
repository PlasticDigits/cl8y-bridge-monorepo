import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test/setup.ts',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules/', 'dist/'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src/test/',
        '**/*.d.ts',
        'src/main.tsx',
        'src/vite-env.d.ts',
      ],
      thresholds: {
        // Utility functions should have high coverage
        'src/utils/': {
          statements: 80,
          branches: 70,
          functions: 80,
          lines: 80,
        },
      },
    },
    // Timeout for tests (useful for integration tests)
    testTimeout: 10000,
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
})
