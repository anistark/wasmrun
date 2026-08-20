#!/usr/bin/env bash
# Agent API flow: run a TypeScript project's tests, with an ESM npm dependency,
# a tsconfig, and a failing assertion that names the line in the .ts source.
set -euo pipefail

BASE="${WASMRUN_AGENT_URL:-http://localhost:8430}/api/v1"

command -v jq >/dev/null || { echo "jq is required"; exit 1; }

echo "→ creating session"
SESSION_ID=$(curl -sf -X POST "$BASE/sessions" | jq -r .session_id)
echo "  session: $SESSION_ID"

trap 'curl -sf -X DELETE "$BASE/sessions/$SESSION_ID" > /dev/null && echo "→ session destroyed"' EXIT

echo "→ running the project's tests (ESM dependency vendored, tsconfig applied, node:test)"
RESPONSE=$(curl -sf -X POST "$BASE/sessions/$SESSION_ID/exec" \
    -H 'Content-Type: application/json' \
    -d @- <<'EOF'
{
  "files": {
    "tsconfig.json": "{\"compilerOptions\":{\"target\":\"ES2020\",\"baseUrl\":\".\",\"paths\":{\"@app/*\":[\"src/*\"]}}}",
    "src/search.ts": "import escapeStringRegexp from 'escape-string-regexp';\n\nexport function matches(haystack: string, needle: string): boolean {\n  return new RegExp(escapeStringRegexp(needle)).test(haystack);\n}\n\nexport function countMatches(haystack: string, needle: string): number {\n  const pattern = new RegExp(escapeStringRegexp(needle));\n  return (haystack.match(pattern) ?? []).length;\n}\n",
    "tests/search.test.ts": "import { test } from 'node:test';\nimport assert from 'node:assert';\nimport { matches, countMatches } from '@app/search';\n\ntest('treats metacharacters literally', () => {\n  assert.equal(matches('price is $5.00', '$5.00'), true);\n  assert.equal(matches('price is $5x00', '$5.00'), false);\n});\n\ntest('counts every occurrence', () => {\n  assert.equal(countMatches('a.b.c', '.'), 2);\n});\n"
  },
  "entry": "tests/search.test.ts",
  "language": "typescript",
  "dependencies": { "escape-string-regexp": "^5.0.0" },
  "timeout": 300
}
EOF
)

echo "$RESPONSE" | jq -r '.stdout, (.error // empty)'

EXIT_CODE=$(echo "$RESPONSE" | jq -r .exit_code)
echo "→ exit_code: $EXIT_CODE"
if [ "$EXIT_CODE" = "0" ]; then
    echo "  unexpected: 'counts every occurrence' is meant to fail"
else
    echo "  non-zero, so an agent knows the suite failed without parsing the TAP"
fi
