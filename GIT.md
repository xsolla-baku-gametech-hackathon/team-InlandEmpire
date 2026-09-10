# Git for TapPad

Two long-lived branches. `main` is always demo-ready. `dev` is where work
lands. Nobody commits to either directly.

```
main     <- merge from dev at each checkpoint, tagged, by C only
dev      <- pull requests from feature branches, by anyone
feat/*   <- one task each, short-lived
```

## Names

Branch: `<type>/<area>-<short>`. Types: `feat`, `fix`, `docs`, `test`.
Areas: `protocol`, `server`, `bridge`, `game`, `firmware`, `ci`.

```
feat/server-mock-provider
feat/game-shop-page
fix/bridge-mac-serial-path
docs/xsolla-setup
```

Commit message: imperative, under 60 characters, says what changed, not "wip".

```
Add per-tap spending limit to registry
Poll order status every 800ms
Fix serial path on macOS
```

Tags: `v0.1.0-mock`, `v0.2.0-sandbox`, `v0.3.0-demo`, `v1.0.0`.

## Daily flow

```
git checkout dev && git pull
git checkout -b feat/server-mock-provider
# work, commit small
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git push -u origin feat/server-mock-provider
gh pr create --base dev --fill
```

One teammate reads the PR. Checks: tests added, docs updated, no `unwrap` in
library code, no secrets, CI green. Then merge with a merge commit. Squash
throws away commits, and commits are counted for a prize.

```
gh pr merge --merge
```

## Checkpoint flow, C only

```
git checkout dev && git pull
cargo test --workspace            # must pass
git checkout main && git pull
git merge --no-ff dev -m "Release v0.1.0-mock"
git tag v0.1.0-mock
git push origin main --tags
```

Never force-push `main` or `dev`. Never delete a branch, merged or not: the
history of who did what stays visible. Never commit `.env`.

## Rules that keep commits countable and honest

- Commit every time something runs that did not run before. That is the
  natural rhythm, and it is 30 to 50 real commits in a day.
- One thing per commit. Formatting-only commits are fine, but say so.
- No empty commits, no split-a-line-into-ten commits. The rules penalise it and
  the reviewers can see it.
- Every commit is by a human name. No tool names, no generated-by lines.

## Conflicts

Small branches merged often rarely conflict. When one does, the branch owner
rebases on `dev`, resolves, and re-runs the three commands above.

```
git fetch origin && git rebase origin/dev
```
