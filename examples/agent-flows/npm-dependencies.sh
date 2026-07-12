#!/usr/bin/env bash
# Agent API flow: execute JavaScript that depends on a pure-JS npm package.
# The package is vendored host-side (the sandbox has no network).
set -euo pipefail

BASE="${WASMRUN_AGENT_URL:-http://localhost:8430}/api/v1"

command -v jq >/dev/null || { echo "jq is required"; exit 1; }

echo "→ creating session"
SESSION_ID=$(curl -sf -X POST "$BASE/sessions" | jq -r .session_id)
echo "  session: $SESSION_ID"

trap 'curl -sf -X DELETE "$BASE/sessions/$SESSION_ID" > /dev/null && echo "→ session destroyed"' EXIT

echo "→ executing with dependencies: { lodash: ^4.17.21 }"
curl -sf -X POST "$BASE/sessions/$SESSION_ID/exec" \
    -H 'Content-Type: application/json' \
    -d @- <<'EOF' | jq -r '.stdout, (.error // empty)'
{
  "source": "const _ = require('lodash'); console.log('pairs=' + JSON.stringify(_.toPairs({a: 1, b: 2}))); console.log('chunked=' + JSON.stringify(_.chunk([1, 2, 3, 4, 5], 2)));",
  "language": "javascript",
  "dependencies": { "lodash": "^4.17.21" },
  "timeout": 120
}
EOF
