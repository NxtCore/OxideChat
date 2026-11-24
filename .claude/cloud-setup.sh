#!/usr/bin/env bash
set -uo pipefail

npx -y skills add cloudflare/security-audit-skill -s security-audit -g -a claude-code --copy -y
npx -y skills add pbakaus/impeccable -s impeccable -g -a claude-code --copy -y
npx -y skills add vercel-labs/agent-skills -s web-design-guidelines -g -a claude-code --copy -y

claude plugin marketplace add DietrichGebert/ponytail
claude plugin install ponytail@ponytail --scope user -y 2>/dev/null || claude plugin install ponytail@ponytail --scope user

npm install -g @colbymchenry/codegraph@1.6.2
claude mcp add --scope user codegraph -- codegraph serve --mcp
if git rev-parse --show-toplevel >/dev/null 2>&1; then
  (cd "$(git rev-parse --show-toplevel)" && codegraph init -y) || true
fi

exit 0
