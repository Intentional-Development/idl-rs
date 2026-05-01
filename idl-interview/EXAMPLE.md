# `idl-interview` — TodoApp demo

End-to-end demonstration that the Wave 9 `idl interview` runtime can drive a
five-round greenfield interview to completion against a deterministic mock
provider, then promote the accumulated kernel delta into a proposed change
folder.

## Demo fixture

Five canned `RoundResponse` documents shipped under
`fixtures/demo-todo-app/round-{1..5}.json`. They replay the few-shot examples
from `IDL/skills/idl-interview/prompts/round-*.md`, normalized to the
schema-conformant `idl://interview/...` anchor URI form.

Aggregate kernel content (deduped by id across all five rounds):

| metric | value |
| --- | --- |
| nodes | **16** |
| edges | **7** |
| node kinds present | `intent`, `scope`, `entity`, `variant`, `operation`, `event`, `state_machine`, `rule`, `access_pattern`, `decision`, `verification` |

This satisfies the "~12 nodes" target from the Wave 9 plan and matches the
13-node TodoApp demo doc (`IDL/skills/idl-interview/demo-todo-app.md`) plus a
few extra `decision` nodes that the prompt few-shots include.

## Reproducing the demo

```bash
# from the workspace root
cd idl-rs
cargo build --bin idl

mkdir -p /tmp/idl-demo/intent/changes
cd /tmp/idl-demo

export IDL_INTERVIEW_MOCK_DIR=$OLDPWD/idl-rs/idl-interview/fixtures/demo-todo-app

idl interview new --topic "todo app for solo users" --rounds 5
SID=$(ls intent/.idl/interview/sessions | head -1)
for n in 2 3 4 5; do idl interview continue "$SID"; done
idl interview accept "$SID"
```

Expected output (verified locally during Wave 9 implementation):

```
✓ session created: sess-XXXXXXXXXX-XXXXXX
  round 1 done (attempts=1, confidence=0.76, questions=1)
✓ round 2 done (attempts=1, confidence=0.83, questions=0)
✓ round 3 done (attempts=1, confidence=0.82, questions=0)
✓ round 4 done (attempts=1, confidence=0.83, questions=0)
✓ round 5 done (attempts=1, confidence=0.86, questions=0)
✓ promoted to 0001-todo-app-for-solo-users (16 nodes, 7 edges) at intent/changes/0001-todo-app-for-solo-users
```

The promoted change folder contains:

```
intent/changes/0001-todo-app-for-solo-users/
  state.json          # state=proposed, transitions[0]={from:draft,to:proposed,by:idl-interview}
  delta.json          # canonical kernel-conformant graph (16 nodes, 7 edges)
  intent-delta.idl    # human-readable pointer to delta.json
  decisions.md        # rendered decision ledger (one section per round)
  sources.json        # flattened source_anchors[] from every node
  verifications/plan.md
  ai-runs/<session-id>.jsonl
```

## Where this is exercised

- `tests/runner_tests.rs::interview_runs_5_rounds_to_completion` — drives all
  five rounds via `MockProvider`, asserts ≥12 accumulated nodes.
- `tests/runner_tests.rs::accept_creates_proposed_change_folder` — promotes
  the accumulated delta and asserts the on-disk artifacts above.
- `tests/runner_tests.rs::invalid_delta_retries_then_fails` and
  `one_invalid_then_recovers` — exercise the bounded retry loop in
  `runner::run_round_with_retries` (max 2 retries → 3 attempts total).

Switching to the real OpenAI Responses API requires only unsetting
`IDL_INTERVIEW_MOCK_DIR` and exporting `OPENAI_API_KEY`; the CLI selects the
provider transparently.
