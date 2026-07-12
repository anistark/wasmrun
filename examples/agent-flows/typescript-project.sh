#!/usr/bin/env bash
# Agent API flow: execute a multi-file TypeScript project in one request.
set -euo pipefail

BASE="${WASMRUN_AGENT_URL:-http://localhost:8430}/api/v1"

command -v jq >/dev/null || { echo "jq is required"; exit 1; }

echo "→ creating session"
SESSION_ID=$(curl -sf -X POST "$BASE/sessions" | jq -r .session_id)
echo "  session: $SESSION_ID"

trap 'curl -sf -X DELETE "$BASE/sessions/$SESSION_ID" > /dev/null && echo "→ session destroyed"' EXIT

echo "→ executing TypeScript project (transpiled in-sandbox, imports resolved by the runtime)"
curl -sf -X POST "$BASE/sessions/$SESSION_ID/exec" \
    -H 'Content-Type: application/json' \
    -d @- <<'EOF' | jq -r '.stdout, (.error // empty)'
{
  "files": {
    "main.ts": "import { circle } from './shapes'; const c = circle(5); console.log(`area=${c.area}`); console.log(`perimeter=${c.perimeter}`);",
    "shapes.ts": "import { TWO_PI } from './constants'; export interface Circle { area: number; perimeter: number } export function circle(r: number): Circle { return { area: Math.PI * r * r, perimeter: TWO_PI * r }; }",
    "constants.ts": "export const TWO_PI = 2 * Math.PI;"
  },
  "entry": "main.ts",
  "language": "typescript",
  "timeout": 120
}
EOF
