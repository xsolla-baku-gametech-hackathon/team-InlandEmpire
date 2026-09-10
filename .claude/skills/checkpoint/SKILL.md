---
name: checkpoint
description: Verify the current branch runs, tick CHECKLIST.md, stage a commit message
disable-model-invocation: true
allowed-tools: Bash(cargo:*), Bash(git status:*), Bash(git diff:*), Bash(git branch:*), Bash(git log:*)
---

## Live state

- Branch: !`git branch --show-current`
- Changes: !`git status --short`
- Last commits: !`git log --oneline -5`

## Steps

1. Run, in this order, and stop at the first failure with its output:
   `cargo fmt --all -- --check`, then
   `cargo clippy --workspace --all-targets -- -D warnings`, then
   `cargo test --workspace`.
   Done means all three printed success.
2. Read `CHECKLIST.md`. For every unticked box whose condition you have seen
   run in this session, tick it. A box you believe is done but have not seen
   run stays unticked; say which ones and why.
3. Check the diff for secrets: `git diff --cached` and `git diff` must not
   contain an API key, a token, or a `.env` file.
4. Print a commit message: imperative, under 60 characters, names the change.
   Below it, the files it would include. No tool names, no generated-by lines.
5. Stop. The human runs `git commit`. If they have said "commit" in this turn,
   commit with that message and nothing else.
