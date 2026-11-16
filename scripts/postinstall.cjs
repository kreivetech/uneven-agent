/**
 * postinstall.js
 *
 * Runs after `npm install uneven-ai`.
 *
 * Responsibilities:
 * - Verify the native binary exists for the current platform
 * - Print a helpful message about the LLM model (optional download)
 *
 * The native binary is bundled in prebuilds/ — no Rust toolchain needed.
 * The LLM model (~1.4 GB) is NOT bundled — too large for npm.
 * Users can run `uneven-ai init` to set up and configure a provider.
 */

const fs = require('fs')
const path = require('path')

const platform = process.platform
const arch = process.arch
const platformKey = `${platform}-${arch}`

const root = path.resolve(__dirname, '..')
const binaryName = platform === 'win32' ? 'uneven.exe' : 'uneven'
const binaryPath = path.join(root, 'bin', platformKey, binaryName)

// ─── Check native binary ──────────────────────────────────────────────────────

const binaryExists = fs.existsSync(binaryPath)

if (!binaryExists) {
  const supported = ['linux-x64', 'linux-arm64', 'darwin-arm64', 'win32-x64']

  if (!supported.includes(platformKey)) {
    console.warn(`\n[uneven-ai] ⚠ Platform '${platformKey}' is not officially supported.`)
    console.warn(`         Supported platforms: ${supported.join(', ')}`)
    console.warn(`         You can build from source: cargo build --release -p uneven-cli\n`)
  } else {
    console.warn(`\n[uneven-ai] ⚠ Native binary for ${platformKey} not found.`)
    console.warn(`         This is unexpected for a supported platform.`)
    console.warn(`         Please report this at https://github.com/kreivesler/uneven/issues\n`)
  }
} else {
  const size = (fs.statSync(binaryPath).size / 1024 / 1024).toFixed(1)
  console.log(`\n[uneven-ai] ✓ Native binary loaded for ${platformKey} (${size} MB)`)
}

// ─── Welcome message ──────────────────────────────────────────────────────────

console.log(`
╔══════════════════════════════════════════════════════════════╗
║  Uneven AI — Autonomous Local Development Agent              ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║  Quick start:                                                ║
║    uneven-ai init          Initialize in your project        ║
║    uneven-ai index         Index your codebase               ║
║    uneven-ai ask "..."     Ask about your code               ║
║                                                              ║
║  Configure your AI provider in uneven.config.ts:             ║
║    provider: 'local'    -> use local LLM (download ~1.4 GB)  ║
║    provider: 'claude'   -> set ANTHROPIC_API_KEY             ║
║    provider: 'openai'   -> set OPENAI_API_KEY                ║
║    provider: 'ollama'   -> start Ollama locally              ║
║                                                              ║
║  Docs: https://github.com/kreivesler/uneven                  ║
╚══════════════════════════════════════════════════════════════╝
`)
