#!/usr/bin/env bash
# Uneven — CLI Demo Script
#
# Demonstrates the full pipeline using the Uneven CLI.
# Run from the project root: bash examples/demo.sh

set -euo pipefail

UNEVEN="node dist/cli/index.js"
KNOWLEDGE_DIR="examples/knowledge"
SEP="────────────────────────────────────────────────────────────"

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  UNEVEN — Autonomous Local Development Agent"
echo "  CLI Demo"
echo "════════════════════════════════════════════════════════════"
echo ""

# Step 1: Check build
if [ ! -f "dist/cli/index.js" ]; then
  echo "[ ! ] Project not built. Running npm run build first..."
  npm run build
fi

# Step 2: Init
echo "[ Step 1 ] Initialize"
echo "$SEP"
$UNEVEN init
echo ""

# Step 3: Index sample knowledge base
echo "[ Step 2 ] Index sample knowledge base"
echo "$SEP"
echo "Indexing: $KNOWLEDGE_DIR"
echo ""
# grep exits 1 when no non-DEBUG lines are found — || true prevents that from
# killing the script while still letting a real index failure propagate via pipefail.
$UNEVEN index --dir "$KNOWLEDGE_DIR" 2>&1 | { grep -v "^\[DEBUG\]" || true; }
echo ""

# Step 4: Ask questions
echo "[ Step 3 ] Ask questions about the indexed knowledge"
echo "$SEP"
echo ""

questions=(
  "What API endpoints does this project have?"
  "How does authentication work in this project?"
  "What security vulnerabilities exist in the codebase?"
  "How should I fix the SQL injection bug?"
)

for q in "${questions[@]}"; do
  echo "❓ $q"
  echo "────────────────────────────────────────────────────────────"
  $UNEVEN ask "$q"
  echo ""
  echo ""
  # /dev/tty allows reading from the terminal even when stdin is redirected;
  # fall back gracefully in non-interactive environments (CI, piped input).
  if [ -t 0 ] || [ -c /dev/tty ]; then
    read -r -p "Press ENTER for next question..." </dev/tty || true
  fi
  echo ""
done

# Step 5: Security scan
echo "[ Step 4 ] Run security analysis on sample files"
echo "$SEP"
# pentest requires explicit targets — show usage hint rather than exiting
$UNEVEN pentest --help 2>/dev/null || echo "  (run: uneven-ai pentest --target <ip/cidr> to scan a real target)"
echo ""

# Step 6: View log
echo "[ Step 5 ] Show activity log"
echo "$SEP"
$UNEVEN log 2>/dev/null | head -40 || true
echo ""

# Done
echo "════════════════════════════════════════════════════════════"
echo "  Demo complete!"
echo ""
echo "  Configure your AI provider in uneven.config.ts:"
echo "    provider: 'claude'   → set ANTHROPIC_API_KEY"
echo "    provider: 'openai'   → set OPENAI_API_KEY"
echo "    provider: 'ollama'   → start Ollama locally"
echo "════════════════════════════════════════════════════════════"
echo ""
