/**
 * Uneven — Interactive Demo
 *
 * This script demonstrates the full Uneven pipeline:
 * 1. Initialize engine
 * 2. Index the sample knowledge base in examples/knowledge/
 * 3. Ask questions and get AI-powered answers
 *
 * Usage:
 *   npx ts-node examples/demo.ts
 *
 * Or with a compiled build:
 *   node dist/examples/demo.js
 */

import { Uneven } from '../src'
import type { UnevenConfig } from '../types'
import * as readline from 'readline'

// ─── Configuration ────────────────────────────────────────────────────────────

const config: UnevenConfig = {
  brain: {
    // Change to 'openai', 'claude', 'gemini', or 'ollama' for real inference
    provider: 'local',
    model: 'llama3.2',
  },
  knowledge: {
    // Index the sample files in this directory
    dirs: ['examples/knowledge'],
  },
  log: {
    path: '.uneven/demo-log.md',
    append: false,
    includeTimestamp: true,
  },
}

// ─── Demo Questions ───────────────────────────────────────────────────────────

const DEMO_QUESTIONS = [
  'What API endpoints does this project expose?',
  'How does authentication work?',
  'What are the known security bugs?',
  'How should I fix the SQL injection vulnerability?',
  'What environment variables does the project need?',
]

// ─── Helpers ─────────────────────────────────────────────────────────────────

function separator(char = '─', width = 60): string {
  return char.repeat(width)
}

function print(msg: string): void {
  process.stdout.write(msg + '\n')
}

async function ask(rl: readline.Interface, prompt: string): Promise<string> {
  return new Promise(resolve => rl.question(prompt, resolve))
}

// ─── Main Demo ────────────────────────────────────────────────────────────────

async function runDemo(): Promise<void> {
  print('')
  print(separator('═'))
  print('  UNEVEN — Autonomous Local Development Agent')
  print('  Interactive Demo')
  print(separator('═'))
  print('')

  const uneven = new Uneven(config)

  // Step 1: Initialize
  print('[ Step 1 ] Initializing engine...')
  await uneven.init()
  print('✓ Engine ready\n')

  // Step 2: Index knowledge base
  print('[ Step 2 ] Indexing knowledge base (examples/knowledge/)...')
  print('  Files: project.md, auth-service.ts, bugs.md')
  await uneven.index()
  print('✓ Knowledge base indexed\n')

  // Step 3: Interactive Q&A
  print('[ Step 3 ] Knowledge Base Q&A')
  print(separator())
  print('')

  const rl = readline.createInterface({ input: process.stdin, output: process.stdout })

  // Wrap all rl usage in try-finally so rl.close() is always called even if
  // an exception escapes mid-demo — an unclosed Interface keeps stdin in raw
  // mode and the event loop alive, causing the process to hang instead of exit.
  try {
    const choice = await ask(
      rl,
      'Run (1) automated demo questions or (2) interactive mode? [1/2]: '
    )

    print('')

    if (choice.trim() === '2') {
      // Interactive mode
      print('Interactive mode — type your question or "exit" to quit.')
      print('')

      while (true) {
        const question = await ask(rl, '❓ Your question: ')

        if (question.toLowerCase() === 'exit' || question.toLowerCase() === 'quit') {
          break
        }

        if (!question.trim()) continue

        try {
          print('')
          const answer = await uneven.ask(question)
          print(separator('-'))
          print(`💡 Answer:\n\n${answer}`)
          print(separator('-'))
          print('')
        } catch (err) {
          print(`✗ Error: ${err instanceof Error ? err.message : String(err)}`)
          print('')
        }
      }
    } else {
      // Automated demo
      for (const question of DEMO_QUESTIONS) {
        print(`❓ ${question}`)
        print(separator('-'))

        try {
          const answer = await uneven.ask(question)
          print(`💡 ${answer}`)
        } catch (err) {
          print(`✗ ${err instanceof Error ? err.message : String(err)}`)
        }

        print('')
        print('')

        // Pause between questions to let user read
        await ask(rl, 'Press ENTER for next question...')
        print('')
      }
    }
  } finally {
    rl.close()
  }

  // Step 4: Summary
  print(separator('═'))
  print('  Demo complete!')
  print('')
  print('  Next steps:')
  print('  • Edit uneven.config.ts and set provider: "claude" or "openai"')
  print('  • Run: uneven index --dir ./src')
  print('  • Run: uneven ask "How does my auth middleware work?"')
  print('  • Run: uneven watch --command "npm run dev"')
  print(separator('═'))
  print('')
}

runDemo().catch(err => {
  console.error('Demo failed:', err instanceof Error ? err.message : String(err))
  process.exit(1)
})
