# Contributing to OxideChat

Thanks for helping out. This page covers how to name and describe your work so that history stays readable. For code style, architecture and testing rules, read [AGENTS.md](AGENTS.md): it applies to humans and AI agents alike.

## Workflow

1. Branch off `master`.
2. Keep one topic per branch. A refactor that the feature needs can live in the same PR; an unrelated fix gets its own.
3. Open a pull request against `master` and fill in the template.
4. CI must be green: Clippy, `cargo test`, generated frontend types, and the Playwright e2e suite.
5. PRs are **squash-merged**, so the PR title becomes the commit subject on `master` and the description becomes its body. Write both for someone reading `git log` a year from now.

## Branch names

```
<type>/<area>-<short-description>
```

All lowercase, words separated by `-`, short enough to type.

| Type       | Use for                                             |
| ---------- | --------------------------------------------------- |
| `feat`     | New user-facing or API behaviour                    |
| `fix`      | A bug fix                                           |
| `refactor` | Restructuring without behaviour change              |
| `perf`     | Performance work                                    |
| `test`     | Tests only (unit or e2e)                            |
| `docs`     | Documentation only                                  |
| `chore`    | Dependencies, CI, build, tooling                    |
| `security` | Security fixes and hardening                        |

The area is the part of the app you touch, e.g. `gateway`, `budgets`, `chat`, `teams`, `developer`, `settings`, `admin`, `i18n`, `e2e`.

Good: `feat/gateway-request-log-ttft`, `fix/budgets-team-pool-refund`, `chore/omniference-0.4.1`
Avoid: `my-changes`, `feat/stuff`, `henrik/test2`, `worktree-agent-a8567881`

Branches created by AI agents (`claude/...`) are fine while they work, but rename them to this scheme before opening a PR.

## PR titles (and commit subjects)

```
[TYPE] - <what changes>
```

`TYPE` is the branch type in capitals: `[FEAT]`, `[FIX]`, `[REFACTOR]`, `[PERF]`, `[TEST]`, `[DOCS]`, `[CHORE]`, `[SECURITY]`. It is followed by a space, a hyphen and a space.

The part after the prefix is one sentence that says **what changes**, written as an instruction:

- Imperative mood: "Add", "Fix", "Remove", "Log", "Let" — not "Added", "Adds" or "Adding".
- Sentence case, no trailing period.
- At most 72 characters including the prefix. If you need "and" twice, the PR probably does too much.
- Plain text: no Markdown (`**bold**`), no emoji.
- Name the behaviour, not the files: "Refund budget reservations when a stream is dropped", not "Update ledger.rs".
- Mention the area when it isn't obvious from the verb and noun.

| Good | Why the alternative is worse |
| --- | --- |
| `[FEAT] - Log finish reason and time to first token on request logs` | — |
| `[FIX] - Hide revoked keys of other teams from team admins` | "[FIX] - fix bug" says nothing |
| `[CHORE] - Bump Omniference to 0.4.1` | "Use omniference 0.4.1" has no prefix |
| `[REFACTOR] - Move OpenAI gateway handlers to src/routes/gateway` | "streamline archistecture issues" is vague and misspelled |
| `[TEST] - Add e2e tests for the settings save bar` | "**Add E2E tests for settings workflows and enhance admin tools**" mixes two topics and uses Markdown |

Commits inside a branch may be messy since they are squashed. If you merge without squashing, every commit subject must follow the same rules.

## PR descriptions

The template in `.github/pull_request_template.md` asks for these sections. Keep each short; delete the ones that don't apply.

- **What** — the change in two or three sentences, from a user's or API client's point of view.
- **Why** — the problem or issue it solves. Link the issue (`Closes #123`).
- **How** — only the parts a reviewer would not guess from the diff: design choices, trade-offs, things you tried and dropped.
- **Breaking changes / migrations** — new migrations, changed API responses, new environment variables, changed permissions. Write "None" when there are none.
- **Testing** — what you ran (`cargo test`, a specific e2e spec, manual steps) and what you did not cover.
- **Screenshots** — for any UI change, before and after, light and dark mode if the change affects colour.

Write the description as plain prose for humans. Do not paste the full diff summary, a generated file list, or an AI chat transcript.

## Before you open the PR

- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` pass.
- `cargo test` passes, and `cargo test generate_frontend_types` leaves no diff in `frontend/types/generated/`.
- New UI features have an e2e spec in `e2e/` (see [e2e/README.md](e2e/README.md)).
- New user-facing strings go through i18n, not hardcoded text.
- New modules or patterns are described in [AGENTS.md](AGENTS.md).
- Migrations are additive and named `<timestamp>_<what_it_does>.sql`, e.g. `20261104120000_add_key_expiry.sql`.
