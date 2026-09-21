# ADR 0001: Remove catch-all CODEOWNERS

Status: **Accepted** — this document is not architecture approval.
Keywords in
[#195](https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/195)
are not architecture approval. Ordinary design is not a founder card.
Merging this design or its implement PR does not authorize deploy, spend,
custody, or agent-policy expansion
([cl8y-agent-control#297](https://git.cl8y.com/PlasticDigits/cl8y-agent-control/issues/297)).
This repo has no `autonomy.rs` / HMAC surface; do not add one to
self-approve.

Date: 2026-09-21

Numbering: first record under `docs/adr/`.
`packages/multichain-rs/security_reviews/ADR_001_ZEROIZE_EVALUATION.md`
is a package-local security evaluation, not this series.

Do **not** wait on leftover-complete or
[cl8y-agent-control#429](https://git.cl8y.com/PlasticDigits/cl8y-agent-control/issues/429)
before implement. **Do** open the leftover tracker (D4) before merge of
#195. Forge protection for `code/*` is already the standing contract
([cl8y-forgejo#48](https://git.cl8y.com/PlasticDigits/cl8y-forgejo/issues/48),
[cl8y-forgejo `docs/INVARIANTS.md`](https://git.cl8y.com/PlasticDigits/cl8y-forgejo/src/branch/main/docs/INVARIANTS.md)).
This ticket is the leftover product-tree delete (forge #48 AC5), not a
second protection rollout.

[code/hello#15](https://git.cl8y.com/code/hello/pulls/15)
is cited only for plant JSON field names and the four Forgejo CODEOWNERS
paths. It is **not** a completed leftover-tracker land: tip `1892075` is
the same incomplete delete as `fc1b20f` here (`CODEOWNERS` still on hello
`main`; no leftover issue). Do not copy its land pattern.

## Outcome

Root catch-all `CODEOWNERS` (`.* @code/maintainers`) is gone so Forgejo
does not plant an official review request on every change. All four
Forgejo CODEOWNERS paths are **absent from the git object** (not empty,
not comments-only, not merely missing from a worktree). Merge to `main`
remains: pull request, no direct push, required Woodpecker context
`ci/woodpecker/pr/woodpecker`, no `force_merge`. Docs in this repo
describe that gate instead of a CODEOWNERS merge block.

**Land vehicle — one recipe.** Update
`chore/remove-catchall-codeowners` **in place**
([PR #195](https://git.cl8y.com/code/cl8y-bridge-monorepo/pulls/195)).
Pinned order: **docs-land** onto
`origin/chore/remove-catchall-codeowners` (copy **only** the three
design docs from the independently accepted **tree**, land D3, apply
the exact Status **line** carve-out, **new commit**, no `--amend` of
`fc1b20f`, `git push` without `--force` unless replacing a verified
bad tip); **then** merge-from-main if `origin/main` is not an ancestor
of that tip; **then one** full boxed checklist immediately before
`Do: merge`. Do not run five-path on a merge-only tip. Do not treat
“before push and before merge” as one gate. When `origin/main` is not
an ancestor (live admin protection `block_on_outdated_branch: true`),
**merge** `origin/main` into the product branch (not rebase, not
squash); keep the `CODEOWNERS` deletion; do **not** `git rm` paths
that exist on `origin/main` (live example: #194 `renovate.json`).
When `origin/main` **is** already an ancestor and
`git diff --name-only origin/main "$TIP"` has extra names, those
extras are **product-tip-only**: do not merge-for-them; do not
`git rm` a path that exists on `origin/main`; drop the product-only
extras from `$TIP` and re-run five-path. Do **not** freeze
[#194](https://git.cl8y.com/code/cl8y-bridge-monorepo/pulls/194)
(`chore: Configure Renovate`) or other PRs. `Do: merge` is a
**merge commit** (`default_merge_style: merge`; do not squash-merge or
rebase-merge #195 onto `main`). `head_commit_id` **is** the SHA that
passed D4’s pre-merge object checklist. Do not merge `fc1b20f` as tip.
Post-merge completion is tree contents on `origin/main`, not a
five-path diff and not descendant-of-`fc1b20f`. Design branch
`cac-design-issue-195` is review/transport only; it is **outside** the
land slices; do not open it as a design-only PR. Full recipe: **D4**.

In this repo the issues URL and the product PR share one number.
Merging PR #195 closes issue #195. Post-merge plant-check therefore
lives on a leftover tracker opened **before** that merge. Do not treat
#195’s own leftover official request as that proof. Plant-check PR
`{n}` is closed **without merge** after its closed-not-merged GET is
pasted, and **before** the leftover issue is closed.

## Context

Today this tree has:

```
# CODEOWNERS (repository root)
.* @code/maintainers
```

Forgejo CODEOWNERS is Go-regexp, not GitHub glob. A non-comment line
matching `^\.\*\s+@\S+` is a catch-all
([cl8y-forgejo `is_catchall_codeowners`](https://git.cl8y.com/PlasticDigits/cl8y-forgejo/src/branch/main/scripts/repo_policy.py)).
Forgejo loads the **first existing** file among `CODEOWNERS`,
`docs/CODEOWNERS`, `.gitea/CODEOWNERS`, and `.forgejo/CODEOWNERS`
(`.gitea/` remains in the walk; `.forgejo/` was added in forgejo#8773).
The last three paths are absent here. A comments-only leftover at an
earlier path is not a catch-all regex match, but it still shadows later
paths; absence on all four is the contract (path names as on hello#15,
not hello#15’s land state).

With a one-person maintainers team, official CODEOWNERS review plus
`block_on_official_review_requests` 405s `Do: merge` / 422s self-approve.
Forge #48 reversed that **protection** half across `code/*` and
`PlasticDigits/*` (`required_approvals: 0`,
`block_on_official_review_requests: false`, `enable_push: false`, check
`ci/woodpecker/pr/woodpecker`). Leaving the file still plants requests
on new PRs and keeps onboarding text false.

Open product PR
[#195](https://git.cl8y.com/code/cl8y-bridge-monorepo/pulls/195)
(`chore/remove-catchall-codeowners`, commit `fc1b20f`) already deletes
the file and nothing else. Live plant on that PR (known leftover; not
S3 evidence) is team `maintainers` / `id: 4`, not the CODEOWNERS token
`@code/maintainers`. Sample JSON is under Observability. That PR GET
has `requested_reviewers: []` (users) while
`requested_reviewers_teams` is non-empty; pass-iff must check both.

`chore/remove-catchall-codeowners` / #195 and the design tree diverge
at `main` `4586378`. The product tip is still `fc1b20f` (delete only,
Woodpecker green, “Can be merged”). Concurrent open PR
[#194](https://git.cl8y.com/code/cl8y-bridge-monorepo/pulls/194)
(`chore: Configure Renovate`) is mergeable on that same `main`. It
adds only `renovate.json`. Two-dot
`git diff --name-only origin/chore/remove-catchall-codeowners origin/renovate/configure`
is `CODEOWNERS` + `renovate.json`. If #194 lands first,
`origin/main` is **not** an ancestor of the product tip. Extra names
in `git diff --name-only origin/main "$TIP"` then include paths that
**exist on `origin/main`** (live: `renovate.json`) — merge
`origin/main` (keep the `CODEOWNERS` deletion); never `git rm` a path
that exists on `origin/main` (that would revert Renovate on
`Do: merge`). After that merge, if docs-land has not run, the
merge-only tip diffs as **`{CODEOWNERS}`** — missing docs is not
“extra paths mean main moved,” and five-path must not run on that
tip. Pinned order is docs-land **then** merge-from-main if needed,
then one land-ready checklist. If `origin/main` **is** already an
ancestor and extras remain, they are **product-tip-only**: do not
merge-for-them (merge is a no-op); do not `git rm` a path that exists
on `origin/main`; drop the product-only extras from `$TIP` and
re-run five-path. Live admin protection has `block_on_outdated_branch: true`
(forge-contract field; **not** a public `GET .../branches/main` key).
If #194 or any other PR lands first, #195 becomes outdated. The allowed
recovery is **merge** `origin/main` into the product branch (D4), not a
freeze of #194, and not a rebase. Repo settings are
`default_merge_style: merge` but `allow_squash_merge: true` (and
rebase); `Do: merge` of #195 onto `main` is a **merge commit**.
Post-merge completion does **not** re-check descendant-of-`fc1b20f`
(tree contents on `origin/main` only). Design-only SHAs still have root
`CODEOWNERS`,
including `8d19560`, `8b5a9f8`, and successors of this ADR while
Status is **Proposed**. Resetting #195 to a design SHA
(`git reset --hard <design-sha>` or `git checkout <design-sha> -- .`)
makes a design-only tip that still plants reviews. Copying **only**
the three docs paths onto the product branch keeps the delete. The
expected **pre-merge `head_commit_id`** is a **descendant of
`fc1b20f`** that keeps that delete and adds docs — not `fc1b20f`
itself, and not a design SHA. Do not require that ancestry after land.

Worktree `test ! -e CODEOWNERS` is not the land predicate. Forgejo
merges a **commit**. `rm CODEOWNERS` in the worktree, then force-push a
design SHA that still has the file, is green on disk and restores the
plant on the PR. Absence is `git ls-tree` / `git cat-file` on the tip
**object**. `git reset --hard <design-sha> && git rm CODEOWNERS &&
commit && push --force` can look like a delete while the tip is **not**
a descendant of `fc1b20f`; the boxed D4 checklist rejects that.

Public `GET /api/v1/repos/code/cl8y-bridge-monorepo/branches/main`
(2026-09-21) already shows `protected: true`, `required_approvals: 0`,
`status_check_contexts: ["ci/woodpecker/pr/woodpecker"]`,
`user_can_push: false`. That payload does **not** attest
`dismiss_stale_approvals` / “Stale reviews dismissed”; drop that row.
D2 flags `block_on_official_review_requests`, `block_on_rejected_reviews`,
`enable_push`, and `block_on_outdated_branch` are forge-contract /
admin PATCH fields, not public `GET .../branches/main` keys. Do not
invent PATCH verification here. `block_on_outdated_branch: true` is
why D4’s allowed update is **merge** `origin/main`, not a freeze.

`docs/qa-onboarding.md` still claims a false gate. Live strings (not
the JSON key `required_approvals: 1`, which is **not** in the file).
Inventory on this design tree (line numbers are evidence **now**; D3
is **not** implemented by frozen line numbers — L428–429 stay put,
L513 and L618 move after the Branch Protection rewrite):

- Heading `## Branch Protection & Merge Rules`: “merge without
  approval”; “**1 approving review**”; “**CODEOWNERS enforced**”
  targeting `@PlasticDigits` (the file actually assigns
  `@code/maintainers`); “**Stale reviews dismissed**” (unattested on
  the public `main` protection GET; drop)
- Bold lead-in `**After creating the PR:**`: substring `wait for the
  maintainer to review` (live file wraps before `yourself.`; match
  unwrapped). Reads as a Forgejo requirement.
- Heading `## Branch & MR Conventions`, Reviews row: “Maintainer
  (`@PlasticDigits`) reviews and merges all MRs”
- Heading `## Useful Commands Reference`, `fj pr view` comment:
  “maintainer merges after approval”

Woodpecker for this repo is root `.woodpecker.yaml` (gitleaks, operator
`writers::` / `rpc_fallback`, frontend hash-verify) posting
`ci/woodpecker/pr/woodpecker`. That check is the merge gate, not
CODEOWNERS.

`docs/qa-onboarding.md` teaches `Fixes #N` on PR bodies (keep that
teaching). `.github/PULL_REQUEST_TEMPLATE.md` does too (`Fixes #123`).
`fj pr create` without `--body` / `--body-file` opens that template;
`--autofill` copies commit messages into the PR body. Forgejo closes
issues from **pull request descriptions, pull request comments, and
commit messages** (including squash/merge messages). A close-keyword
immediately targeting the leftover iid (`Fixes #200`, `closes #200`)
on any of those surfaces on #195 or `{n}` closes the evidence issue.
Bare `#200` is a link, not a close. That teaching is **keep-but-dangerous**
for PR #195 and plant-check `{n}`. Do not strip `Fixes #N` from
onboarding or the template.

## Non-goals

- PATCH Forgejo branch protection, `apply_repo_policy.py`, or migrate
  templates (forge #48 / this-repo has no those scripts). Do not invent
  an admin PATCH verification of `block_on_*` / `enable_push` in this
  product ticket; public `GET .../branches/main` cannot attest those.
- CAC merge-drain, occupying autoland, `DrainSkip::OfficialReview`
  cleanup, or dismissing reviewers from the controller (#429 / #388).
- `force_merge`, `enable_push: true`, dropping
  `ci/woodpecker/pr/woodpecker`, posting fake commit statuses, or moving
  `.woodpecker.yaml` into `.woodpecker/` (out of scope; a sibling forge
  repo hit a directory-preference bug — do not copy that fix here).
- Delete vendor `packages/**/lib/**/.github/CODEOWNERS` (OpenZeppelin,
  forge-std). Those are not Forgejo CODEOWNERS locations.
- Add path-specific CODEOWNERS as a substitute merge gate.
- Change operator, canceler, contract, or frontend runtime behavior.
- Deploy, spend, custody, key rotation, or agent privilege expansion.
- File a founder card for this ordinary chore.
- Open a competing product PR besides #195.
- Merge `fc1b20f` **as the land tip** (the expected **pre-merge
  `head_commit_id`** is a descendant that keeps the delete and adds
  docs; do not require that ancestry after land).
- Push a design-only SHA (still has `CODEOWNERS`, including `8b5a9f8`
  and successors) as the PR #195 tip.
- `git reset --hard <design-sha>` or `git checkout <design-sha> -- .`
  onto `chore/remove-catchall-codeowners`.
- Rebase `chore/remove-catchall-codeowners` onto `main` (or
  squash/rebase-merge) “to keep a linear tip”; that drops
  descendant-of-`fc1b20f`. Allowed outdated-branch update is **merge**
  `origin/main` (keep the `CODEOWNERS` deletion).
- Freeze
  [#194](https://git.cl8y.com/code/cl8y-bridge-monorepo/pulls/194)
  (`chore: Configure Renovate`) or other PRs until #195 lands.
  Implicit freeze is not a recipe.
- `git rm` a path that exists on `origin/main` (live example:
  `renovate.json` after #194) so a name-only gate passes. Extra-path
  recovery splits on the ancestor check: not ancestor → merge
  `origin/main` (keep the `CODEOWNERS` deletion); already ancestor →
  do not merge-for-them; drop **product-only** extras from `$TIP`
  and re-run five-path.
- Run five-path on a merge-only tip (docs-land not yet on `$TIP`).
  After #194 lands, that tip diffs as `{CODEOWNERS}`. Missing docs is
  not “main moved.” Pinned order: docs-land, then merge-from-main if
  needed, then one full boxed checklist immediately before `Do: merge`.
- Treat “before push and before merge” as one five-path gate. That
  blocks the merge-from-main push. Five-path + Status carve-out +
  `architecture.md`/`README.md` byte-identity are the land-ready /
  `head_commit_id` gate only.
- Treat a five-path `git diff --name-only origin/main` vs the merged
  tip as post-merge completion (`origin/main` **is** that tip, so the
  diff is empty after a correct `Do: merge`).
- Treat descendant-of-`fc1b20f` as a **post-merge** audit. Pre-merge
  still requires it on `head_commit_id`; after land, audit tree
  contents on `origin/main` only.
- Squash-merge or rebase-merge PR #195 onto `main` (`allow_squash_merge`
  and rebase are on; land with a **merge commit**).
- Treat worktree `test ! -e` as the pre-merge land predicate.
- Treat PR #195’s own planted request as post-merge plant-check proof.
- Merge plant-check `{n}`; leftover-complete closes `{n}` without merge
  **before** closing the leftover issue.
- Dismiss reviews or `POST .../requested_reviewers` on `{n}`.
- A docs-only PR from `cac-design-issue-195`. That branch transports
  design between VMs; it is not a land slice and not a merge vehicle.
- Wait for a second design SHA only to flip ADR Status. Implement
  applies the Status carve-out on the product branch.
- Copy hello#15’s land pattern (incomplete delete; no leftover tracker).
- Wait on leftover-complete or CAC #429 before starting implement.

## Decision

### D1 — Delete the catch-all file via PR, never direct `main`

Remove root `CODEOWNERS` whose non-comment lines include `.* @…`. Do
not leave a copy at `docs/CODEOWNERS`, `.gitea/CODEOWNERS`, or
`.forgejo/CODEOWNERS`. After land, `git ls-tree <tip> --` each of those
four paths is empty (path missing in the **object**). None may contain
a reviewer rule for any pattern (not only `.*`). Do not leave an empty
or comments-only file at any of those paths (Forgejo still parses the
first existing file and a comments-only root would shadow later paths).
Land through a pull request (`Do: merge` with `head_commit_id` bound
to the D4 invariant-passing SHA). Direct push to `main` stays forbidden.

### D2 — Merge gate is forge protection, not this file

Standing contract (already applied; this PR must not change it).
Evidence classes are not one GET. Do not invent an admin PATCH
verification of forge-contract rows in this product ticket.

**Public `GET /api/v1/repos/code/cl8y-bridge-monorepo/branches/main`**
(attested 2026-09-21):

| Field | Value |
| --- | --- |
| `required_approvals` | `0` |
| `status_check_contexts` | `["ci/woodpecker/pr/woodpecker"]` |
| `user_can_push` | `false` |

**Forge-contract** (admin PATCH fields; **not** public `GET
.../branches/main` keys):

| Flag | Value |
| --- | --- |
| `enable_push` | `false` |
| `block_on_official_review_requests` | `false` |
| `block_on_rejected_reviews` | `true` |
| `block_on_outdated_branch` | `true` |

**Merge API:**

| Flag | Value |
| --- | --- |
| `force_merge` | never |

A later PR that re-adds CODEOWNERS does not restore the official-review
**block** unless an admin PATCHes protection (forge invariant 9).

### D3 — Docs match the gate; residual approval-as-gate copy too

Implement rewrites `docs/qa-onboarding.md` so it does not describe
approval as a Forgejo merge requirement. Anchor by **heading + exact
current substring** (and by presence of the replacement strings below).
Do **not** implement D3 by frozen line numbers: the Branch Protection
rewrite leaves the `**After creating the PR:**` block in place and
shifts the Reviews row and `fj pr view` comment.

Keep: PRs to `main`, Woodpecker `ci/woodpecker/pr/woodpecker`, no
direct push, no deleting `main`. Drop approving-review and CODEOWNERS
as protection. Label maintainer review as **practice**. Drop “Stale
reviews dismissed”: it is not attested on the public `main` protection
GET used above. Keep “never merge your own MRs” as practice; do not
write it as if `required_approvals` were 1. Keep QA `Fixes #N`
teaching (onboarding + `.github/PULL_REQUEST_TEMPLATE.md`); it is
keep-but-dangerous for #195 and `{n}`.

Ban list (none of these substrings may remain in
`docs/qa-onboarding.md`): `merge without approval`, `1 approving
review`, `CODEOWNERS enforced`, `after approval`, `wait for the
maintainer to review`, `reviews and merges all MRs`. Do **not** use
absence of the JSON key `required_approvals: 1` as the test — that
key is not in the file.

Replacement copy (implement lands this, not the design commit).
Prescribed replacements must not reintroduce the ban strings.

**`## Branch Protection & Merge Rules`** — current substrings include
`merge without approval`, `1 approving review`, `CODEOWNERS enforced`,
and `Stale reviews dismissed`. Replace that section with:

> **Important:** Our default branch is `main`, **not** `master`. When creating pull requests, always target `main`. QA devs occasionally target `master` by mistake — double-check the target branch before submitting.
>
> `main` is protected. You **cannot** push directly to it.
>
> | Rule | Effect |
> | --- | --- |
> | **PRs required** | All changes to `main` must go through a pull request |
> | **Woodpecker PR check** | Merge requires green `ci/woodpecker/pr/woodpecker` |
> | **No force pushes** | Force-pushing to `main` is blocked |
> | **No branch deletion** | `main` cannot be deleted |
>
> Maintainer review of QA/frontend PRs is **practice**, not Forgejo
> `required_approvals`. Official CODEOWNERS review is not a merge gate.
> Merge-gate copy: [ADR 0001](adr/0001-remove-catchall-codeowners.md);
> forge contract: [docs/INVARIANTS.md](https://git.cl8y.com/PlasticDigits/cl8y-forgejo/src/branch/main/docs/INVARIANTS.md).
>
> **What this means for you:** create a branch, push it, open a PR
> targeting `main`, and wait for `ci/woodpecker/pr/woodpecker`. You
> should **never** merge your own MRs — the maintainer reviews and
> merges them as operational practice.

**Bold lead-in `**After creating the PR:**`** — anchor on that heading
plus substring `wait for the maintainer to review` (unwrapped; live
file wraps before `yourself.`). Keep “Do not merge it yourself” as
practice; wait for Woodpecker, not a required approval:

> **After creating the PR:** wait for `ci/woodpecker/pr/woodpecker`.
> Do not merge it yourself (practice, not `required_approvals`). If
> review comments come in, push fixes and the PR updates automatically:

**`## Branch & MR Conventions`, Reviews row** — current substring:
`reviews and merges all MRs`:

> | Reviews | Maintainer (`@PlasticDigits`) reviews and merges MRs as **practice**, not a Forgejo approval gate |

**`## Useful Commands Reference`, `fj pr view` comment** — current
substring: `maintainer merges after approval`. The replacement must
not contain `after approval`:

> `fj pr view                # View your PR details (maintainer merges as practice)`

### D4 — One product PR; leftover tracker contract; in-place recipe

Do not open a competing delete PR. Transport on `cac-design-issue-195`
is **outside** this recipe (review/inter-VM only). The only land
recipe, on PR #195. Pinned order (not merge-before-docs, not
five-path-before-every-push): docs-land onto
`origin/chore/remove-catchall-codeowners` → merge-from-main if
`origin/main` is not an ancestor → wait for
`ci/woodpecker/pr/woodpecker` → **one** full boxed checklist
immediately before `Do: merge`.

1. On `chore/remove-catchall-codeowners` (PR #195), copy **only** these
   three blobs from the independently accepted design **tree** (not from
   `git show` of a single design commit — `docs/README.md` is on the
   tree; it may be absent from the tip commit’s diff, as with
   `git show 8d19560` omitting the README index that landed in
   `51488f9`):
   - `docs/adr/0001-remove-catchall-codeowners.md`
   - `docs/architecture.md`
   - `docs/README.md`
   Required form: `git checkout <accepted-sha> --` those three paths.
   Do **not** `git reset --hard <design-sha>`. Do **not**
   `git checkout <design-sha> -- .`. Do **not** use
   `cac-design-issue-195` as the PR tip.
2. Keep the `CODEOWNERS` delete already on the branch.
3. Land D3 in `docs/qa-onboarding.md`.
4. **Status carve-out.** On this ADR only, replace **exactly** this
   line (no global replace of the word `Proposed`):

   ```text
   - Status: **Proposed** — this document is not architecture approval.
   + Status: **Accepted** — this document is not architecture approval.
   ```

   Byte-diff of the three design blobs vs `<accepted-sha>` allows only
   that token. Every other byte of those three blobs stays identical
   to the accepted tree. Do not wait for a second design SHA only to
   flip Status.
5. **New commit** (no `--amend` of `fc1b20f`) for the docs land;
   `git add` only the four docs paths on that commit; `git push`
   **without** `--force`. Feature-branch `--force` is allowed **only**
   to replace a verified bad tip; on that SHA about to be pushed, run
   four-path object absence + descendant-of-`fc1b20f` +
   `TIP != fc1b20f` (not five-path). Force-push of `main` is not. Do
   not rebase onto `main` (or squash/rebase-merge) “to keep a linear
   tip”; that drops descendant-of-`fc1b20f`. Do **not** run five-path
   as a docs-land push gate. The allowed-shaped force-push
   `git reset --hard 8b5a9f8 && git push --force` is **forbidden**: it
   restores catch-all planting (this design tree still has root
   `CODEOWNERS`; so did `8d19560`).
6. **Then** merge-from-main if `origin/main` is not an ancestor of the
   docs-landed tip. Switch from
   `origin/chore/remove-catchall-codeowners` (see snippet; do not
   `-C` over an unpushed docs-land). Resolve `CODEOWNERS` by keeping
   the deletion. Do **not** `git rm` paths that exist on
   `origin/main`. Do **not** run five-path as a merge-from-main push
   gate. Wait for `ci/woodpecker/pr/woodpecker`. Do **not** freeze
   #194. `Do: merge` of #195 onto `main` is a **merge commit** (not
   squash or rebase-merge).
7. Never push a design-only SHA (root `CODEOWNERS` still present,
   including `8d19560`, `8b5a9f8`, and successors) as the PR tip.
8. Never merge `fc1b20f` **as tip**. “Never merge `fc1b20f`” means that
   commit as tip, not “never merge a descendant.” The expected
   **pre-merge** `head_commit_id` is a descendant of `fc1b20f` that
   keeps the delete and adds docs. Do not merge `fc1b20f`. After land,
   do **not** re-check descendant-of-`fc1b20f` on `origin/main` (tree
   contents only). **One** full boxed checklist immediately before
   `Do: merge` is the land-ready / `head_commit_id` gate (ancestor +
   five-path + object absence + Status **line** carve-out +
   `architecture.md`/`README.md` byte-identity).

**Docs-land update (FF from current product tip; first in the pinned
order).** Does **not** require `origin/main` to be an ancestor.
Five-path is not a docs-land push gate:

```bash
git fetch origin
git switch --detach origin/chore/remove-catchall-codeowners
# or: git switch -C chore/remove-catchall-codeowners origin/chore/remove-catchall-codeowners
# `-C` only when local is not ahead of that origin tip. If `-C` fails
# because another worktree holds the name, stay on `--detach` and
# `git push origin HEAD:chore/remove-catchall-codeowners`.
git checkout <accepted-sha> -- \
  docs/adr/0001-remove-catchall-codeowners.md \
  docs/architecture.md \
  docs/README.md
# land D3 in docs/qa-onboarding.md (heading + substring anchors)
# Status line only (exact carve-out above)
git add \
  docs/adr/0001-remove-catchall-codeowners.md \
  docs/architecture.md \
  docs/README.md \
  docs/qa-onboarding.md
git commit -m "docs: land catch-all CODEOWNERS delete and merge-gate copy"
# new commit; do not --amend fc1b20f
# commit message: no close-keyword immediately targeting leftover iid
git push origin HEAD:chore/remove-catchall-codeowners
# no --force unless replacing a verified bad tip
```

**Allowed update-from-main (merge, not a freeze; after docs-land).**
Live admin protection has `block_on_outdated_branch: true`. Concurrent
PR [#194](https://git.cl8y.com/code/cl8y-bridge-monorepo/pulls/194)
(`chore: Configure Renovate`) is open and mergeable on the same `main`
as #195 (`4586378`) and **may land first**. Do **not** wait for a
freeze of #194. Do **not** run merge-from-main before docs-land (that
merge-only tip diffs as `{CODEOWNERS}` after #194; five-path must not
run on it). After docs-land and `git fetch origin`, if `origin/main`
is not an ancestor of the product tip:

```bash
git fetch origin
# Same fallback as docs-land. Never `-C` onto origin/chore/… if this
# HEAD already descends from that origin tip (would discard unpushed
# docs-land and recreate a merge-only tip).
if git merge-base --is-ancestor origin/chore/remove-catchall-codeowners HEAD; then
  : # already on/ahead of the origin product tip; merge in place
else
  git switch --detach origin/chore/remove-catchall-codeowners
  # `-C` only when local is not ahead of that origin tip:
  # git switch -C chore/remove-catchall-codeowners origin/chore/remove-catchall-codeowners
  # If `-C` fails because another worktree holds the name, stay on
  # `--detach` and `git push origin HEAD:chore/remove-catchall-codeowners`.
fi
# product branch only; never reset --hard to a design SHA.
# Do not `git switch chore/remove-catchall-codeowners` — this worktree
# may have no local product branch (fails, or merges into a stale local
# tip, then a non-FF push looks like a “verified bad tip” force-push).
git merge origin/main
# keep CODEOWNERS deletion; no rebase/squash; no close-keyword on leftover iid
# If CODEOWNERS conflicts (deleted by us / modified by them): keep the
# deletion (`git rm CODEOWNERS`). Do **not** `git rm` a path that
# exists on origin/main (live example: renovate.json from #194).
git push origin HEAD:chore/remove-catchall-codeowners
# no --force unless replacing a verified bad tip
```

Then wait for `ci/woodpecker/pr/woodpecker`. Do **not** run five-path
as a merge-from-main push gate. The **one** full boxed checklist
(ancestor + five-path + object absence + Status **line** carve-out +
`architecture.md`/`README.md` byte-identity) runs immediately before
`Do: merge`.

**Product-tip invariant — one land-ready checklist on the fetched tip
SHA, immediately before `Do: merge`.** Run on
`TIP=$(git rev-parse origin/chore/remove-catchall-codeowners)`
(the **object**, not the worktree). This is **not** a before-push
gate: do not run five-path before the docs-land push or the
merge-from-main push. If force-push is used to replace a verified bad
tip, run four-path object absence + descendant-of-`fc1b20f` +
`TIP != fc1b20f` on the SHA about to be pushed; five-path + Status
carve-out + `architecture.md`/`README.md` byte-identity stay the
land-ready / `head_commit_id` gate. Do **not** use post-merge ancestry
or a post-merge five-path diff as the land predicate (`origin/main`
**is** the landed tip, so that diff is empty). `head_commit_id` **is**
this SHA. `Do: merge` is a **merge commit** of it (not
squash/rebase-merge onto `main`).

- `git merge-base --is-ancestor fc1b20fee585a6e541d6c211c5b6cf4d5a4db4a7 "$TIP"`
  (keeps descendant-of-`fc1b20f`; rejects `git reset --hard <design-sha>`
  and other non-descendant tips even when the object lacks `CODEOWNERS`).
- `test "$TIP" != fc1b20fee585a6e541d6c211c5b6cf4d5a4db4a7`
- Tip is not a design-only SHA that still has root `CODEOWNERS`
  (`8d19560`, `8b5a9f8`, successors of this ADR while Status is
  **Proposed**).
- Four Forgejo CODEOWNERS paths missing in the **object** (not the
  worktree):

  ```bash
  for p in CODEOWNERS docs/CODEOWNERS .gitea/CODEOWNERS .forgejo/CODEOWNERS; do
    test -z "$(git ls-tree "$TIP" -- "$p")"
  done
  ```

  `git cat-file -e "$TIP:$p"` must fail for each path.
  Worktree `test ! -e` is not sufficient.
- After `git fetch origin`, require `origin/main` to be an ancestor of
  `$TIP` **before** the five-path equality check. The ancestor stop is
  a command (`|| { echo …; exit 1; }`), not a comment: do not run
  five-path yet if it fails. Two extra-path branches:

  1. Not ancestor → run the allowed update-from-main **merge** (keep
     the `CODEOWNERS` deletion). Do **not** `git rm` paths that exist
     on `origin/main` (live: #194 `renovate.json`). Do not run
     five-path yet.
  2. Already ancestor → do not merge-for extra names (merge is a
     no-op). Do **not** `git rm` a path that exists on `origin/main`.
     Extra names are **product-tip-only**; drop them from `$TIP` and
     re-run five-path.

  Only after `origin/main` is an ancestor, require **exact sorted
  equality** to these five paths vs **then-current** `origin/main`:

  ```bash
  git fetch origin
  git merge-base --is-ancestor origin/main "$TIP" || {
    echo "origin/main is not an ancestor of $TIP; run update-from-main (keep CODEOWNERS deletion). Do not git rm extra paths. Do not run five-path yet."
    exit 1
  }
  test "$(git diff --name-only origin/main "$TIP" | sort)" = "$(printf '%s\n' \
    CODEOWNERS \
    docs/adr/0001-remove-catchall-codeowners.md \
    docs/architecture.md \
    docs/README.md \
    docs/qa-onboarding.md | sort)"
  # if this test fails after ancestor passed: extras are product-tip-only;
  # do not merge-for-them; do not git rm a path that exists on origin/main;
  # drop product-only extras from $TIP and re-run five-path
  ```
- `git diff <accepted-sha> "$TIP" --` the three design blobs allows
  only the Status line token (`Proposed` → `Accepted` on that one
  line). `docs/architecture.md` and `docs/README.md` are
  byte-identical to `<accepted-sha>`.

**Leftover tracker** — `fj issue create`, not a PR. Open it **before**
`Do: merge` of #195. Leftover-complete is: paste the record (both GET
pairs, `updated_at` / `head.sha` stability, `{n}` closed-not-merged
GET) → `{n}` already closed **without merge** → close that **issue**.
It is not a merge.

**Forgejo close-keyword grammar.** A close is a close-keyword
immediately targeting the leftover iid. Keywords (case-insensitive):
`fix`, `fixes`, `fixed`, `close`, `closes`, `closed`, `resolve`,
`resolves`, `resolved`. Examples that close: `Fixes #200`,
`closes #200`. Bare `#200` is a **link**, not a close. Do **not** treat
any `#<iid>` as a close.

**Forbidden surfaces** on **both** PR #195 and plant-check `{n}`: PR
title, PR description, PR comments, **commit messages**,
**squash/merge messages**, and `fj pr create --autofill` bodies (and
template bodies from omitting `--body`). D4 creates commits on
`chore/remove-catchall-codeowners`; those messages must not close the
tracker. Onboarding still teaches `Fixes #N`; that is keep-but-dangerous
here.

Paste:

```text
fj issue create --body-file leftover-tracker.md --no-template \
  "chore: leftover plant-check after catch-all CODEOWNERS delete"
```

`leftover-tracker.md` body (fields to fill later; `{n}` is the
dedicated plant-check PR):

```markdown
Post-merge evidence for removing catch-all CODEOWNERS.

Merging PR #195 closes issue #195; this tracker survives.

Close-keyword immediately targeting this leftover iid (`Fixes #<this-iid>`,
`closes #<this-iid>`) is forbidden on PR title, description, comments,
commit messages, squash/merge messages, and `fj pr create --autofill`
bodies of both PR #195 and plant-check `{n}`. Bare `#<this-iid>` is a
link, not a close. Do not merge `{n}`.

Leftover-complete:
1. Paste `{n}` + **both** GET pairs (pair 1 + pair 2) + `updated_at` /
   head-SHA stability + four-path object check on `main` below.
2. Close plant-check PR `{n}` **without merge** (`fj pr close {n}`; no
   `-w` containing a close-keyword immediately targeting this iid).
3. Paste `GET .../pulls/{n}` showing `state == "closed"` and
   `merged == false`.
4. Close this leftover **issue**.

## Record

- plant-check PR `{n}`:
- **Pair 1** (immediate post-open; must pass all jq-able clauses; left
  unmutated across the wait — no undraft / retitle / push / dismiss /
  `POST .../requested_reviewers`):
  - `GET /api/v1/repos/code/cl8y-bridge-monorepo/pulls/{n}` body.
    Must include `draft`, `title`, `changed_files`,
    `requested_reviewers`, `requested_reviewers_teams`, plus
    `updated_at` and head SHA (`head.sha`):
  - `GET /api/v1/repos/code/cl8y-bridge-monorepo/pulls/{n}/reviews`
    body:
- **Pair 2** (after one 30s wait; **pass snapshot** / pass decision;
  same field set as pair 1):
  - `GET /api/v1/repos/code/cl8y-bridge-monorepo/pulls/{n}` body
    (must include `draft`, `title`, `changed_files`,
    `requested_reviewers`, `requested_reviewers_teams`, plus
    `updated_at` and head SHA):
  - `GET /api/v1/repos/code/cl8y-bridge-monorepo/pulls/{n}/reviews`
    body:
- Stability across the wait: pair-2 PR GET `updated_at` equals pair-1
  PR GET `updated_at`, and pair-2 `head.sha` equals pair-1 `head.sha`
  (proves pair 1 was taken, passed, and left unmutated):
- four-path object check on `main`
  (`for p in CODEOWNERS docs/CODEOWNERS .gitea/CODEOWNERS .forgejo/CODEOWNERS; do test -z "$(git ls-tree origin/main -- "$p")"; done`):
- `GET /api/v1/repos/code/cl8y-bridge-monorepo/pulls/{n}` after
  `fj pr close {n}` (`state == "closed"`, `merged == false`):
```

**#195 must not contain a close-keyword immediately targeting the
leftover iid** on any forbidden surface (title, description, comments,
commit messages, squash/merge messages, `--autofill` bodies).

**`{n}` pin.** After the delete is on `main`, open one dedicated
plant-check PR. Branch from post-delete `main`. One throwaway path
that is **not** a Forgejo CODEOWNERS path. Explicit `--body`; no
template; no `--autofill`.

```bash
git fetch origin
git switch -c leftover/plant-check origin/main
printf '%s\n' 'plant-check' > docs/leftover-plant-check.txt
git add docs/leftover-plant-check.txt
git commit -m "chore: leftover plant-check after catch-all delete"
git push -u origin HEAD
fj pr create "chore: leftover plant-check after catch-all delete" \
  --body "Dedicated plant-check after catch-all CODEOWNERS delete. Do not merge."
```

Do **not** omit `--body` (editor / `.github/PULL_REQUEST_TEMPLATE.md`
contains `Fixes #123`). Do **not** pass `--autofill`. The throwaway
path must not be `CODEOWNERS`, `docs/CODEOWNERS`, `.gitea/CODEOWNERS`,
or `.forgejo/CODEOWNERS`. Title, body, and commit message must not
contain a close-keyword immediately targeting the leftover iid. Do
not prefix the title with `WIP: ` (that drafts the PR).

Close without merge, **before** closing the leftover issue:

```bash
fj pr close {n}
```

Do **not** pass `-w` / `--with-msg` whose text contains a close-keyword
immediately targeting the leftover iid. Paste `GET .../pulls/{n}`
showing `state == "closed"` and `merged == false` **before** closing
the leftover issue.

### D5 — Catch-all definition vs absence

Treat as catch-all any CODEOWNERS at the four Forgejo paths whose
non-comment line matches `^\.\*\s+@\S+`. Comments-only files are **not**
catch-alls under that regex. Still require **absence** on all four
paths so a comments-only root file cannot shadow a later path. A
path-scoped line such as `docs/.* @code/maintainers` is **not** this
ticket; do not add one.

## Component / state / interface changes

| Surface | Change |
| --- | --- |
| `CODEOWNERS` | Delete. No replacement file at the four Forgejo paths |
| `docs/CODEOWNERS`, `.gitea/CODEOWNERS`, `.forgejo/CODEOWNERS` | Must remain absent (no empty or comments-only leftover) |
| `docs/adr/0001-remove-catchall-codeowners.md` | This decision (design branch Status **Proposed**; implement copies it and applies the Status **line** carve-out; every other byte stays identical) |
| `docs/architecture.md` | Short merge-gate pointer; no duplicated land recipe; byte-identical on implement |
| `docs/README.md` | Index the ADR; byte-identical on implement |
| `docs/qa-onboarding.md` | Land D3 (heading + current-substring anchors); implement, not this commit |
| `.woodpecker.yaml` | Unchanged |
| Branch protection JSON | Unchanged (admin-only; already rolled out) |
| Vendor lib CODEOWNERS | Unchanged |
| Runtime packages | Unchanged |
| Leftover tracker issue | `fj issue create` before merge of #195; owns plant-check; closed as an issue after `{n}` is closed without merge |
| Plant-check PR `{n}` | Post-merge probe from post-delete `main`; close **without merge** before leftover-issue close |

## Affected invariants

This repo has no `docs/INVARIANTS.md` for forge merge. The contract is
cl8y-forgejo invariants **2–10** and **13** (no direct `main`, Woodpecker
PR context, `required_approvals: 0`, official CODEOWNERS review not a
gate, rejected reviews still block, never `force_merge`, do not plant
CODEOWNERS, spoof CODEOWNERS is not a block, land still needs a merge
token + green PR check, Renovate cannot push `main`).

Unchanged here: operator writer invariants INV-OP-W1–W10, frontend /
Terra / Solana bridge invariants, watchtower delay/cancel model.

`docs/qa-onboarding.md` currently **contradicts** forge invariants 4 and
5. Implement removes that contradiction. CAC invariant 67 (never
`force_merge` / do not dismiss CODEOWNERS from the controller) is
untouched; this product PR is the allowed file delete, not a controller
workaround.

## Alternatives

| Option | Why not |
| --- | --- |
| Keep the file; rely on protection `block_on_official_review_requests=false` | Still plants official requests; onboarding stays wrong; forge AC5 leftover |
| Replace `.*` with GitHub-glob `*` | Forgejo is regexp; still a request file; not the policy |
| Path-specific CODEOWNERS for `packages/` | Substitute merge gate; out of scope |
| Dismiss the self-request on every PR | Forbidden CAC substitute (#388); does not scale |
| `force_merge` to skip the 405 | Forbidden |
| Direct-push delete | Violates `enable_push: false` |
| Delete vendor `.github/CODEOWNERS` | Wrong location; unrelated third-party metadata |
| Second PR besides #195 | Duplicate AC5 work; split file vs docs |
| Leave comments-only `CODEOWNERS` | Not a catch-all regex match, but first-existing-file still shadows later paths; absence is the contract |
| Merge `fc1b20f` then follow up with docs | Incomplete land; no ADR/onboarding on the merged tip. `head_commit_id` must be the invariant-passing SHA |
| `git reset --hard` / `git checkout <design-sha> -- .` then force-push | Matches an allowed-shaped feature-branch force-push but restores root `CODEOWNERS` (`8b5a9f8`, `8d19560`, successors); copy only the three docs paths |
| `git reset --hard <design-sha> && git rm CODEOWNERS && commit` | Worktree and even object can look deleted; tip is not a descendant of `fc1b20f`; boxed checklist rejects it |
| Worktree `test ! -e` as land predicate | `rm` locally then force-push a design SHA restores the plant; check `ls-tree` on the SHA |
| Rebase onto `main` / squash “to keep a linear tip” | Drops descendant-of-`fc1b20f`; allowed outdated-branch update is **merge** `origin/main` (keep the `CODEOWNERS` deletion) |
| Freeze #194 / other PRs until #195 lands | Implicit freeze is not a recipe; merge `origin/main` (keep the delete) |
| vs-`fc1b20f` name-only as the land file-set gate | Dead-ends when `main` advances under `block_on_outdated_branch`; use vs-`origin/main` (`CODEOWNERS` + four docs) **after** `origin/main` is an ancestor of `$TIP` |
| Five-path vs `origin/main` while `origin/main` is not an ancestor | Live extra-path example: #194 `renovate.json`. Two-dot vs a concurrent branch is `CODEOWNERS` + `renovate.json`. Not ancestor → merge `origin/main` (keep the delete); do not `git rm` paths that exist on `origin/main`; do not run five-path yet |
| Merge-from-main **before** docs-land, then five-path | After #194 that merge-only tip diffs as `{CODEOWNERS}`. Missing docs is not “main moved.” Pinned order: docs-land, then merge-from-main if needed, then one land-ready checklist |
| Five-path before every push **and** before merge | Blocks the merge-from-main push. Five-path + Status carve-out + `architecture.md`/`README.md` byte-identity are the land-ready / `head_commit_id` gate only |
| `git rm` every extra name in `git diff --name-only origin/main "$TIP"` | After ancestor is true, extras are product-tip-only; merge is a no-op; `git rm` of those files blocks recovery. Split: not ancestor → merge, do not `git rm` origin/main paths; already ancestor → drop product-only extras from `$TIP`, do not `git rm` origin/main paths |
| Five-path `git diff --name-only origin/main` after land as completion | After a correct `Do: merge`, `origin/main` **is** the tip, so that diff is empty; auditor following it fails a correct land. Pre-merge only |
| Post-merge descendant-of-`fc1b20f` while squash/rebase-merge of #195 is allowed | Repo default is merge but squash/rebase are allowed. Keeping post-merge ancestry without pinning merge-commit is inconsistent. This ADR **drops post-merge ancestry** (tree contents on `origin/main` only) and pins merge-commit `Do: merge` as the **land recipe**, not as a post-merge descendant audit |
| `git switch chore/remove-catchall-codeowners` after fetch | Local product branch may be missing or stale; a non-FF push then looks like a verified-bad-tip force-push. Prefer `git switch --detach origin/chore/remove-catchall-codeowners`. `-C` only when local is not ahead of that origin tip |
| `git switch -C … origin/chore/…` after unpushed docs-land | Discards the docs commit and recreates a merge-only tip. If `HEAD` already descends from that origin tip, `git merge origin/main` without `-C`. If `-C` fails because another worktree holds the name, `--detach` + `git push origin HEAD:chore/remove-catchall-codeowners` |
| Boxed ancestor check as a comment, then `test` five-path | Without `set -e`, failed ancestor still five-path-fails on `renovate.json` and invites `git rm`. Ancestor stop is `\|\| { echo …; exit 1; }` |
| Paste only pair 2 on leftover-complete | Cannot audit fail-closed pair 1 or the unmutated wait; record both pairs; pair 2 is the pass snapshot |
| Reset #195 to a design SHA (`8d19560`, `8b5a9f8`, or successor) | Design-only tip; root `CODEOWNERS` still plants reviews |
| Treat hello#15 as the land pattern | Same incomplete delete (`1892075`); CODEOWNERS still on hello `main`; no leftover issue |
| Treat #195’s planted request as plant-check | Merge closes #195; leftover is the evidence |
| Close-keyword immediately targeting leftover iid on #195 or `{n}` | Closes the leftover tracker; D4 forbids it on title, description, comments, commit messages, squash/merge messages, and `--autofill` bodies |
| Merge plant-check `{n}` | Leftover-complete closes `{n}` without merge; merging it is how `Fixes #N` habit hits the tracker |
| Wait 30s after a first GET that fails jq-able clauses 1–3 | Mutation in the window pass-opens leftover-complete; fail on first GET, open a new probe |
| Wait for a second design SHA only to flip Status | Implement applies the Status **line** carve-out on the product branch |
| Global replace of token `Proposed` | Mutates Context / tables / failure modes; only the Status line changes |

## Complexity added / removed

Removed: catch-all official review request on every change; false
onboarding claim that CODEOWNERS / required approval is the merge gate.

Added: this ADR, a short architecture pointer, a leftover tracker
issue that survives merge of #195, and a plant-check `{n}` closed
without merge. No runtime modules, no new CI workflow, no protection
script in this repo.

## Migration

Open PRs may already have an official team `maintainers` request from
the current file (PR #195 does). Protection no longer 405s on that
request. New PRs after the delete do not plant it. No database,
contract, or operator state. No job-status rewrite.

In-flight #195 is the vehicle. Follow **D4** only:

1. Open the leftover tracker with `fj issue create` (D4 paste) **before**
   merge. Do not wait on leftover-complete or #429 before implement.
2. Update `chore/remove-catchall-codeowners` **in place** via the D4
   recipe (docs-land **first**; **then merge** `origin/main` if it is
   not an ancestor; keep the `CODEOWNERS` deletion). Extra-path
   recovery splits on ancestor: not ancestor → merge, do not `git rm`
   paths that exist on `origin/main` (live: #194 `renovate.json`);
   already ancestor → do not merge-for-them; drop product-only extras
   from `$TIP`. Do **not** run five-path before the docs-land or
   merge-from-main push. **One** full boxed checklist immediately
   before `Do: merge` (ancestor as a command that exits 1 if false,
   then five-path + object absence + Status **line** +
   `architecture.md`/`README.md` byte-identity). `head_commit_id` is
   that SHA. Never merge `fc1b20f` as tip. Never push a design SHA as
   the PR tip. Do not freeze #194. `Do: merge` is a **merge commit**.
3. `cac-design-issue-195` is never a land slice and never the merge
   vehicle.
4. #195 and `{n}` must not contain a close-keyword immediately
   targeting the leftover tracker iid on any forbidden surface. Close
   `{n}` without merge (`fj pr close {n}`; no `-w` with leftover iid),
   paste closed-not-merged GET, then close the leftover issue.

## Observability

After land on `main`, audit **tree contents** on `origin/main` (same
list as Integration post-merge). Do **not** require a five-path
`git diff --name-only origin/main` after land (`origin/main` is the
landed tip; that diff is empty). Do **not** require
descendant-of-`fc1b20f` after land (that ancestry is the pre-merge
`head_commit_id` gate only). Require: four-path `ls-tree` empty; this
ADR Status **Accepted**; `docs/architecture.md` and `docs/README.md`
present; D3 strings in `docs/qa-onboarding.md`; vendor
`.github/CODEOWNERS` under `packages/contracts-evm/lib/` unchanged;
`.woodpecker.yaml` still at repo root. Woodpecker still posts
`ci/woodpecker/pr/woodpecker` on PR tips. `fj pr status` / the PR
checks tab remain the merge signal. Do not invent a new metric. Do not
call live Forgejo owner APIs from this repo’s CI.

Plant-check (dedicated post-merge PR, leftover tracker). Fail-closed
on **both** GET pairs. jq-able pass-iff (all required). Pass iff **all**
of:

1. PR GET `draft == false`
2. PR GET `title` does not contain `WIP` (case-insensitive)
3. PR GET `changed_files >= 1` (this field on the PR GET; do **not**
   imply a files GET + Go `.*` — every path matches `.*`)
4. `(requested_reviewers // []) | length == 0`
5. `(requested_reviewers_teams // []) | length == 0`
6. no review with `official == true && state == "REQUEST_REVIEW"`
   (user **or** team)

Process-only (not numbered pass-iff; not a jq field): do **not**
`POST .../requested_reviewers` on `{n}`; do **not** dismiss reviews.
JSON proof of “no reviewers requested” is clauses 4–5 on **both** GET
pairs.

A jq script of clauses 4–6 alone pass-opens a draft/WIP/empty probe.
Clauses 1–3 are in this same list.

`maintainers` / `id: 4` is the known-plant example (live #195), not
the only fail. Do not require `team.organization` on the reviews GET.
Do not match `team.name == "code/maintainers"` or the CODEOWNERS token
`@code/maintainers` (those miss the live plant).

`official` alone means assigned/write-access, not “planted by
CODEOWNERS”; the conjunction with `state == "REQUEST_REVIEW"` is the
plant signal. `REQUEST_REVIEW` is `state`, not a sibling key.

Known-plant sample: PR #195 (must fail this predicate; not leftover
evidence). Live user list is empty (`requested_reviewers: []`); the
team plant is in `requested_reviewers_teams`. Checking only teams
would ignore a user plant; checking only `requested_reviewers` would
pass #195.

`GET /api/v1/repos/code/cl8y-bridge-monorepo/pulls/195` fragment
(shape **each** leftover PR GET paste must include: `draft`, `title`,
`changed_files`, `requested_reviewers`, `requested_reviewers_teams`,
plus `updated_at` and `head.sha` on the live pull object; known-plant
sample below is the plant fields only):

```json
{
  "draft": false,
  "title": "chore: remove catch-all CODEOWNERS",
  "changed_files": 1,
  "requested_reviewers": [],
  "requested_reviewers_teams": [
    {
      "id": 4,
      "name": "maintainers",
      "organization": { "id": 4, "name": "code" }
    }
  ]
}
```

`GET /api/v1/repos/code/cl8y-bridge-monorepo/pulls/195/reviews` fragment:

```json
[
  {
    "id": 196,
    "official": true,
    "state": "REQUEST_REVIEW",
    "team": { "id": 4, "name": "maintainers", "organization": null }
  }
]
```

On #195, `requested_reviewers_teams[0].name == "maintainers"` and
`.organization.name == "code"`. On reviews, `team.name == "maintainers"`,
`team.id == 4`, and `team.organization == null`. Matching the file
string `@code/maintainers` misses the plant. A user-shaped review with
`official == true && state == "REQUEST_REVIEW"` (no `team`) also fails.

`{n}` is the dedicated plant-check PR opened after the delete is on
`main` (D4 pin). Procedure:

1. GET both endpoints immediately after open (pair 1). Do not wait
   first. Capture PR GET fields needed for the leftover record:
   `draft`, `title`, `changed_files`, `requested_reviewers`,
   `requested_reviewers_teams`, plus `updated_at` and `head.sha`, and
   the reviews GET.
2. If **any** jq-able clause (1–6) fails on pair 1 → **fail**; open a
   new probe. Do not wait. A first GET that is draft / `WIP` /
   `changed_files < 1` / non-empty reviewers **fails**; it does not
   wait for a second pair.
3. If pair 1 passes all jq-able clauses: **no mutation** between GETs
   (no undraft, retitle, push, dismiss, `POST .../requested_reviewers`).
   Wait once 30 seconds. GET both endpoints (pair 2), same field set.
4. Pair 2 must satisfy **all** jq-able clauses. If any fail → fail;
   open a new probe. Also require pair-2 PR GET `updated_at` equals
   pair-1 `updated_at` and pair-2 `head.sha` equals pair-1 `head.sha`;
   if either drifted → fail (mutation or concurrent update in the
   wait); open a new probe.
5. Pass only if **both** pairs satisfy all jq-able clauses **and** the
   `updated_at` / `head.sha` stability check holds. Paste **both**
   pairs into the leftover record (pair 1 proves immediate post-open
   pass and the unmutated wait; **pair 2** is the pass snapshot). Each
   PR GET paste includes `draft`, `title`, `changed_files`,
   `requested_reviewers`, `requested_reviewers_teams`, `updated_at`,
   and `head.sha`, plus the reviews GET.
6. Close `{n}` without merge. Paste `GET .../pulls/{n}` with
   `state == "closed"` and `merged == false`. Then close the leftover
   **issue**.

PR #195’s own official request does not pass. “The next natural PR”
does not pass.

## Failure modes

| Failure | Behavior |
| --- | --- |
| File deleted, leftover official request on an old PR | Protection already does not block; optional human dismiss; CAC must not dismiss as policy |
| PR re-adds catch-all CODEOWNERS | Plants requests again; does **not** restore official-review merge block without admin PATCH |
| Comments-only leftover at root / `.gitea/` / `.forgejo/` / `docs/` | First existing file still wins and shadows later paths; `ls-tree` on the tip is non-empty; `git grep` for `.* @` can pass |
| `git grep '^\.\* @'` as the land check | After a correct delete, that command exits **1** (`set -e` false-fail). A comments-only leftover is a grep pass. Land predicate is four-path `ls-tree` empty on the tip **object** |
| Worktree `test ! -e` as pre-merge check | `rm CODEOWNERS` locally then force-push a design SHA is green on disk and restores the plant; reject; check `ls-tree` / `cat-file` on `TIP` |
| Docs still say “CODEOWNERS enforced” / “1 approving review” / “merge without approval” / “after approval” / “reviews and merges all MRs” | Operators treat dismiss-as-reviewer as required; implement must land all of D3, including the Reviews replacement |
| Tests only grep `required_approvals: 1` | False-pass: that JSON key is not in `docs/qa-onboarding.md` |
| D3 Tests allow “replacements **or** protection claims gone” | Table-only surgery leaves Reviews as a gate; require the distinctive replacement phrases |
| Pass-iff ignores `requested_reviewers` | User plant or UI-requested users would pass; live #195 has `[]` so teams-only looks green while Tests say fail if users were requested |
| Pass-iff only fails `team.name == "maintainers"` | Misses a user `official == true && state == "REQUEST_REVIEW"`; fail any such review |
| Plant-check matches `@code/maintainers` | Misses live team `maintainers` / `id: 4`; use the jq-able pass-iff |
| Treating #195’s plant as leftover-complete | Merge closes #195; leftover tracker is the evidence |
| Close-keyword immediately targeting leftover iid on #195 or `{n}` | Closes the evidence issue; forbidden on title, description, comments, commit messages, squash/merge messages, and `--autofill` / template bodies |
| `fj pr create` without `--body` or with `--autofill` for `{n}` | Default template / commit list can inject `Fixes #<tracker>`; use explicit `--body` |
| `fj pr close {n} -w` containing leftover iid | Comment-close of the leftover issue; close without `-w` targeting that iid |
| Merging `{n}` | Leftover-complete closes `{n}` without merge; reject a merge of the probe |
| Closing leftover issue before `{n}` closed-not-merged GET | No proof `{n}` was not merged; paste `state == "closed"` and `merged == false` first |
| Merging `fc1b20f` as tip | Land without ADR/onboarding; reject. `head_commit_id` is the invariant-passing SHA |
| `git reset --hard <design-sha>` or `git checkout <design-sha> -- .` | Restores root `CODEOWNERS`; reject even if the force-push shape is otherwise allowed |
| `git reset --hard <design-sha> && git rm CODEOWNERS && commit && push --force` | Object can lack CODEOWNERS while tip is not a descendant of `fc1b20f`; boxed checklist rejects it |
| Rebase onto `main` then force-push “to keep a linear tip” | Drops descendant-of-`fc1b20f`; allowed update is **merge** `origin/main` (keep the `CODEOWNERS` deletion); do not freeze #194 |
| vs-`fc1b20f` name-only after `main` advances | Merge of `origin/main` makes that diff include unrelated landed paths; land file-set gate is five-path vs **then-current** `origin/main` **after** `origin/main` is an ancestor of `$TIP` |
| Five-path vs `origin/main` without an ancestor command that exits | Live extra-path example: #194 `renovate.json`. A comment after `git merge-base --is-ancestor` still five-path-fails without `set -e` and invites `git rm`. Stop with `\|\| { echo …; exit 1; }`. Not ancestor → merge `origin/main` (keep the `CODEOWNERS` deletion); do not `git rm` origin/main paths; do not run five-path yet |
| Extra paths vs `origin/main` after ancestor is true treated as “main moved” | Merge is a no-op; forbidding `git rm` of product-only files blocks recovery. Drop product-only extras from `$TIP`; do not `git rm` a path that exists on `origin/main` |
| `git rm` a path that exists on `origin/main` (e.g. `renovate.json`) to pass five-path | Reverts landed work on `Do: merge`. Never delete origin/main paths to make the gate pass |
| Five-path on a merge-only tip, or “before push and before merge” as one gate | After #194, merge-only diffs as `{CODEOWNERS}`. Missing docs is not main moved. Docs-land first; five-path is the land-ready gate only |
| Five-path `git diff --name-only origin/main` vs the merged tip as Integration | After a successful `Do: merge`, that diff is **empty**; an auditor fails a correct land. Pre-merge only; post-merge is tree contents (not five-path, not descendant-of-`fc1b20f`) |
| Post-merge descendant-of-`fc1b20f` while squash/rebase-merge of #195 is allowed | `allow_squash_merge` and rebase are on. This ADR **drops post-merge ancestry** (tree contents on `origin/main` only) and pins merge-commit `Do: merge` as the land recipe, not as a post-merge descendant audit |
| `git switch chore/remove-catchall-codeowners` in a worktree with no local product branch | Command fails or merges into a stale local tip; non-FF push looks like a verified-bad-tip force-push. Prefer `--detach origin/chore/remove-catchall-codeowners`. `-C` only when local is not ahead |
| `git switch -C … origin/chore/…` after unpushed docs-land | Discards the docs commit and recreates a merge-only tip. If `HEAD` already descends from that origin tip, `git merge origin/main` without `-C`. If `-C` fails because another worktree holds the name, `--detach` + `git push origin HEAD:chore/remove-catchall-codeowners` |
| Land-ready re-run of ancestor + five-path + object absence only | A tip that merged main, copied docs, and left Status **Proposed** can pass that abbreviated set. Name Status **line** carve-out + `architecture.md`/`README.md` byte-identity in the land-ready checklist |
| Pushing a design SHA as the PR tip | Design-only tip still plants reviews; reject (`8b5a9f8` and successors included) |
| Land Status still **Proposed**, or global replace of `Proposed` | `main` would deny the accepted gate doc, or mutate the ADR; implement applies the Status **line** carve-out only |
| `Do: merge` with `head_commit_id` = `fc1b20f` | API-legal incomplete land; bind `head_commit_id` to the invariant-passing SHA |
| Pass-iff jq of plant-signal clauses only, then wait | Draft / `WIP` / empty first GET still waits; mutate in the window (undraft, retitle, push, dismiss) and leftover-complete pass-opens; **both** pairs must satisfy all jq-able clauses; fail any of 1–6 on pair 1 → fail, new probe |
| Mutation between GET pairs | Forbids dismiss / `POST .../requested_reviewers` / undraft / retitle / push; fail; open a new probe |
| `updated_at` or `head.sha` differs across the two PR GETs | Mutation or concurrent update in the wait; fail; new probe. Leftover record must include both pairs so an auditor can see pair 1 was taken, passed, and left unmutated |
| Pair-1 GET omitted from leftover record | Auditor cannot tell pair 1 was taken, passed, or left unmutated; record both pairs; pair 2 is the pass snapshot |
| Copying hello#15’s land | Incomplete delete; no leftover tracker |
| Woodpecker context missing | Merge stays blocked on the **status check**; do not `force_merge` |
| Implement moves `.woodpecker.yaml` while a `.woodpecker/` dir exists | Woodpecker 3.x may ignore the root file; **do not** do that in this ticket |
| Vendor CODEOWNERS deleted | Wrong blast radius; reject |
| Direct push to `main` | Forbidden; `enable_push` stays false |
| Competing second delete PR | Close or supersede; keep a single #195 lineage |
| Plant-check is draft, title contains `WIP`, or `changed_files < 1` | Fail on the **first** GET; open a new probe |

## Ordered implementation slices

**Transport is outside these slices.** `cac-design-issue-195` carries
the independently reviewed design between VMs. It is not Slice 1, not
a merge vehicle, and not the PR #195 tip. Existence of a design SHA
does not mean the in-place copy is done.

Land on PR #195 only. Slice 1 is the D4 in-place recipe on that
product branch. Slices 1–3 land in the same implement PR. Slice 4
opens before merge of #195 and completes after. Do not wait on
leftover-complete or #429 before implement; **do** open the tracker
before merge of #195.

1. **D4 in-place copy on PR #195** — docs-land on
   `chore/remove-catchall-codeowners` **first**. Copy **only** the three
   docs paths (`git checkout <accepted-sha> --` those paths). Keep the
   delete. Land D3. Apply the Status **line** carve-out. `git add`
   only those four docs paths on the docs-land commit. **New commit**
   (no `--amend` of `fc1b20f`). Push without `--force` unless replacing
   a verified bad tip. Do **not** run five-path as a docs-land push
   gate. **Then**, if `origin/main` is not an ancestor, **merge**
   `origin/main` (keep the `CODEOWNERS` deletion; `--detach` fallback;
   do not `-C` over unpushed docs-land); do not rebase; do not freeze
   #194. Not ancestor → do not `git rm` paths that exist on
   `origin/main` (live: #194 `renovate.json`). Already ancestor → do
   not merge-for extra names; drop product-only extras from `$TIP`. Do
   not run five-path as a merge-from-main push gate. Wait for
   `ci/woodpecker/pr/woodpecker`. Do not `reset --hard`. Do not
   `checkout <design-sha> -- .`. Do not merge `fc1b20f` as tip. **One**
   full boxed checklist **immediately before** `Do: merge` (ancestor
   command that exits 1 if false, then exact sorted five-path vs
   **then-current** `origin/main`, object absence, Status **line**,
   `architecture.md`/`README.md` byte-identity). `head_commit_id` is
   that SHA. `Do: merge` is a **merge commit**.
2. **Delete catch-all** — already on the product branch; confirm
   four-path `ls-tree` empty on the product tip **object** after Slice
   1 (wrong copy methods restore the file). Reuse #195; do not add a
   second PR.
3. **Onboarding copy** — land all of D3 in `docs/qa-onboarding.md`
   by heading + current substring (not frozen line numbers). Remove
   the ban-list strings: `merge without approval`, `1 approving
   review`, `CODEOWNERS enforced`, `after approval`, `wait for the
   maintainer to review`, `reviews and merges all MRs`. Drop “Stale
   reviews dismissed”. Keep: PRs to `main`, Woodpecker
   `ci/woodpecker/pr/woodpecker`, no force-push, no deleting `main`,
   QA `Fixes #N` teaching. “Never merge your own MRs” stays
   **practice**. Distinctive replacements must be present (Tests).
4. **Leftover tracker** — `fj issue create` with the D4 title and body
   **before** merging #195. It owns the dedicated post-merge
   plant-check PR (D4 `{n}` pin). Do not use #195 itself. #195 and
   `{n}` must not contain a close-keyword immediately targeting that
   iid on any forbidden surface. Leftover-complete: paste the record
   (both GET pairs, `updated_at` / `head.sha` stability, `{n}`
   closed-not-merged GET) → leftover **issue** close (after `{n}` is
   already closed without merge).
5. **Verify** — one land-ready boxed checklist on the product tip
   **immediately before** `Do: merge` (ancestor of `$TIP` as a command
   that exits 1 if false, then exact sorted five-path vs then-current
   `origin/main`; Status **line** carve-out;
   `docs/architecture.md` / `docs/README.md` byte-identity; four-path
   object absence). Extra-path recovery splits on ancestor: not
   ancestor → merge `origin/main`, do not `git rm` origin/main paths;
   already ancestor → do not merge-for-them, drop product-only extras
   from `$TIP`, do not `git rm` origin/main paths. Onboarding string
   checks including required replacement phrases; do not touch vendor
   lib CODEOWNERS; do not edit `.woodpecker.yaml`. After land, tree
   contents on `origin/main` only: four-path `ls-tree` empty, ADR
   Status **Accepted**, D3 strings present, vendor CODEOWNERS
   unchanged, `.woodpecker.yaml` at root — do **not** re-run
   five-path or descendant-of-`fc1b20f` as post-merge completion.

## Tests

No runtime tests. Verification is tree assertions in the implement PR
(local or a one-shot script in CI is optional; do not add a new required
Woodpecker workflow name).

Land predicate (four-path **object** absence). After a correct delete
this exits 0 on the fetched tip SHA. A comments-only leftover fails it;
`git grep` for a catch-all line does not. Worktree `test ! -e` is not
this predicate.

```bash
TIP=$(git rev-parse origin/chore/remove-catchall-codeowners)
for p in CODEOWNERS docs/CODEOWNERS .gitea/CODEOWNERS .forgejo/CODEOWNERS; do
  test -z "$(git ls-tree "$TIP" -- "$p")"
done
```

If a grep stays as a secondary catch-all scan, invert it so `set -e`
does not false-fail, and use the catch-all regex. After a correct
delete, uninverted `git grep -nE '^\.\* @' -- CODEOWNERS docs/CODEOWNERS
.gitea/CODEOWNERS .forgejo/CODEOWNERS` exits **1**. A comments-only
file is a grep pass — grep is not the land predicate.

```bash
! git grep -nE '^\.\*\s+@\S+' -- CODEOWNERS docs/CODEOWNERS .gitea/CODEOWNERS .forgejo/CODEOWNERS
```

Do not whole-tree `git grep` (this ADR will match after a correct
delete).

| Requirement | Check |
| --- | --- |
| No Forgejo CODEOWNERS at any load path | Four-path `ls-tree` empty on the tip **object** (land predicate). `git cat-file -e "$TIP:$p"` fails for each path. Worktree `test ! -e` is not sufficient |
| Product-tip invariant | **Pre-merge / `head_commit_id` only** on fetched `TIP`: descendant of `fc1b20f`; `TIP != fc1b20f`; not a design-only SHA; four-path object absence; after `git fetch origin`, ancestor stop is a command that exits 1 if `origin/main` is not an ancestor of `$TIP` (do not run five-path yet). Extra-path recovery: not ancestor → merge `origin/main`, keep the `CODEOWNERS` deletion, do not `git rm` paths that exist on `origin/main` (live: #194 `renovate.json`); already ancestor → do not merge-for extra names, do not `git rm` origin/main paths, drop product-only extras from `$TIP` and re-run five-path. Then exact sorted `git diff --name-only origin/main $TIP` equals the five paths vs **then-current** `origin/main`; Status **line** carve-out; `docs/architecture.md` and `docs/README.md` byte-identical; `head_commit_id` is that `TIP`. Run this checklist **immediately before** `Do: merge`, not before the docs-land or merge-from-main push. **Post-merge:** tree contents on `origin/main` only (four-path absence, Status **Accepted**, D3, vendor CODEOWNERS, `.woodpecker.yaml`). Do **not** require five-path or descendant-of-`fc1b20f` after land |
| D4 copy method | Pinned order: docs-land first (`git checkout <accepted-sha> --` the three docs paths; D3; Status line only; `git add` only those four docs paths on that commit; **new commit**, no `--amend` of `fc1b20f`; no five-path push gate); **then** merge-from-main if `origin/main` is not an ancestor (`--detach origin/chore/remove-catchall-codeowners`; `-C` only when local is not ahead of that origin tip; if `HEAD` already descends from it, `git merge origin/main` without `-C`; if `-C` fails because another worktree holds the name, `--detach` + `git push origin HEAD:chore/remove-catchall-codeowners`; not rebase, not squash); keep the `CODEOWNERS` deletion; do not freeze #194; do not `git rm` origin/main paths; wait for `ci/woodpecker/pr/woodpecker`; **one** full boxed checklist immediately before `Do: merge` (ancestor command + five-path + object absence + Status **line** + `architecture.md`/`README.md` byte-identity). Push without `--force` unless replacing a verified bad tip. No `reset --hard` to a design SHA. No `checkout <design-sha> -- .`. Design branch is not the PR tip. Force-push (bad-tip replace only) re-runs four-path absence + descendant-of-`fc1b20f` + `TIP != fc1b20f` on the SHA about to be pushed. `Do: merge` of #195 onto `main` is a **merge commit** |
| Status carve-out | Exact line: `Status: **Proposed** — this document is not architecture approval.` → `Status: **Accepted** — this document is not architecture approval.` Not a global replace of `Proposed` |
| Vendor CODEOWNERS not deleted | `packages/contracts-evm/lib/forge-std/.github/CODEOWNERS` and OpenZeppelin sibling still exist |
| Onboarding not a false gate | `docs/qa-onboarding.md` contains none of: `merge without approval`, `1 approving review`, `CODEOWNERS enforced`, `after approval`, `wait for the maintainer to review`, `reviews and merges all MRs`. Drop `Stale reviews dismissed` (unattested). Do **not** treat absence of the JSON key `required_approvals: 1` as the test — that key is not in the file. Anchor `**After creating the PR:**` on that heading + unwrapped `wait for the maintainer to review` |
| Residual approval-as-gate copy | D3 replacements **must** contain: `ci/woodpecker/pr/woodpecker`; `practice, not \`required_approvals\``; `not a Forgejo approval gate`; `maintainer merges as practice`. No OR-gate. Do not use frozen line numbers. “never merge your own MRs” remains practice, not `required_approvals`. |
| QA `Fixes #N` teaching | `docs/qa-onboarding.md` still teaches `Fixes #N` on PR bodies (keep). `.github/PULL_REQUEST_TEMPLATE.md` likewise stays. Keep-but-dangerous for #195 and `{n}` |
| Onboarding names the real check | `docs/qa-onboarding.md` mentions `ci/woodpecker/pr/woodpecker` |
| Woodpecker PR workflow still at root | `.woodpecker.yaml` still present; `when` still includes `pull_request` |
| No `force_merge` recommended | grep of the changed docs |
| Protection untouched | implement diff contains no protection JSON / forge admin scripts |
| Close-keyword contract on #195 | No close-keyword immediately targeting leftover iid (`Fixes #<tracker>`, `closes #<tracker>`) on PR title, description, comments, commit messages, squash/merge messages, or `--autofill` bodies. Bare `#<tracker>` is a link, not a close |
| Close-keyword contract on `{n}` | Same forbidden surfaces. `{n}` created with explicit `--body`, no template, no `--autofill`. Throwaway path is not a Forgejo CODEOWNERS path. Branch is from post-delete `main` |
| `{n}` disposal | `fj pr close {n}` without `-w` containing leftover iid; paste `GET .../pulls/{n}` with `state == "closed"` and `merged == false` **before** closing the leftover issue |
| Leftover GET-pair record | Both pairs pasted (same PR GET fields + reviews on each). Pair 2 is the pass snapshot. Pair-2 `updated_at` and `head.sha` equal pair 1 |

Plant-check (leftover tracker, after delete is on `main`):

1. Open `{n}` per D4 pin (post-delete `main`, one non-CODEOWNERS
   throwaway path, `fj pr create` with explicit `--body`, no
   `--autofill`). Title, description, comments, commit messages,
   squash/merge messages, and `--autofill` bodies must not use a
   close-keyword immediately targeting the leftover tracker.
2. GET `.../pulls/{n}` and `.../pulls/{n}/reviews` immediately after
   open (pair 1). Capture `draft`, `title`, `changed_files`,
   `requested_reviewers`, `requested_reviewers_teams`, `updated_at`,
   `head.sha`, plus the reviews GET.
3. If any jq-able clause (Observability 1–6) fails on pair 1 → fail;
   open a new probe. Do not wait. Do not mutate.
4. If pair 1 passes: no mutation (no dismiss, no
   `POST .../requested_reviewers`, no undraft/retitle/push). Wait once
   30 seconds. GET both (pair 2). Pair 2 must satisfy all jq-able
   clauses **and** pair-2 `updated_at` / `head.sha` must equal pair 1.
   Fail otherwise; new probe.
5. Record `{n}`, **both** GET pairs (PR GET on each pair must include
   `draft`, `title`, `changed_files`, `requested_reviewers`,
   `requested_reviewers_teams`, plus `updated_at` and `head.sha`, plus
   the reviews GET), with **pair 2** as the pass snapshot, proof that
   `updated_at` and `head.sha` did not change between the two PR GETs,
   and the four-path `ls-tree` on `origin/main`. Close `{n}` without
   merge. Paste `GET .../pulls/{n}` with `state == "closed"` and
   `merged == false`. Then close the leftover **issue**. Do not use
   #195. Do not use “the next natural PR.” Do not merge `{n}`.

Do not call live Forgejo owner APIs from this repo’s CI. Do not treat
`cargo test` as a merge-gate test for this chore.

## Rollout

1. Independent design review of the published
   `cac-design-issue-195` commit (author cannot approve). Transport
   only; not a land slice.
2. Open the leftover tracker with `fj issue create` (D4 title + body)
   before merge of #195.
3. Implement on #195 via the D4 recipe: copy **only** the three docs
   paths (`git checkout <accepted-sha> --` those paths), keep the
   delete, land D3, apply the Status **line** carve-out, `git add`
   only those four docs paths on the docs-land commit, **new commit**,
   push without `--force` unless replacing a verified bad tip. Do not
   run five-path as a docs-land push gate. **Then**, if `origin/main`
   is not an ancestor, **merge** `origin/main` from the product tip
   (`--detach`; `-C` only when local is not ahead; if `HEAD` already
   descends from `origin/chore/remove-catchall-codeowners`, merge
   without `-C`); keep the `CODEOWNERS` deletion; do not rebase; do
   not freeze #194; do not `git rm` paths that exist on `origin/main`
   (live: #194 `renovate.json`). Wait for
   `ci/woodpecker/pr/woodpecker`. **One** full boxed checklist on the
   fetched tip SHA **immediately before** `Do: merge` (ancestor
   command + five-path + object absence + Status **line** +
   `architecture.md`/`README.md` byte-identity). `head_commit_id`
   **is** that SHA. **Never merge `fc1b20f` as tip.**
4. Confirm #195 has no close-keyword immediately targeting the leftover
   iid on title, description, comments, commit messages, or
   squash/merge messages. Merge with merge-commit `Do: merge` (not
   squash/rebase-merge) and `head_commit_id` = the invariant-passing
   SHA. No `force_merge`. No reviewer dismiss from CAC. After land, do
   **not** require a five-path diff or descendant-of-`fc1b20f` on
   `origin/main` (tree contents only).
5. On the leftover tracker, open `{n}` per D4 pin. Run Observability
   fail-closed both-pair pass-iff. Confirm `{n}` forbidden surfaces
   have no close-keyword immediately targeting the tracker. Paste
   `{n}`, **both** GET pairs (pair 2 is the pass snapshot),
   `updated_at` / `head.sha` stability, and four-path `ls-tree` on
   `main`. `fj pr close {n}` (no `-w` with leftover iid). Paste
   `state == "closed"` and `merged == false`. Close the leftover
   **issue**. Do not treat #195’s own leftover request as that proof.

Chat/issue text does not PATCH protection or deploy.

## Rollback

Restore `CODEOWNERS` **via PR** if a catch-all is wanted again (it
should not be). Restoring the file re-plants requests; it does not by
itself restore `block_on_official_review_requests`. Do not rollback by
PATCHing protection to official-review true or `required_approvals: 1`
from this product ticket. Reverting only the onboarding table leaves the
file-absent state intact and is the smaller docs rollback.

## Integration completion criteria

### Land (merge of PR #195 or successor)

**Pre-merge / `head_commit_id`** (fetched product `TIP`
**immediately before** `Do: merge`; this SHA **is** `head_commit_id`.
Not a before-push gate; do not run five-path before the docs-land or
merge-from-main push):

- Descendant of `fc1b20f`; `TIP != fc1b20f`; not a design-only SHA
  (still has root `CODEOWNERS`, including `8d19560`, `8b5a9f8`, and
  successors while Status is **Proposed**).
- Four-path object absence on `$TIP` (`ls-tree` empty;
  `git cat-file -e "$TIP:$p"` fails). Worktree `test ! -e` is not
  enough.
- After `git fetch origin`: `origin/main` **is** an ancestor of `$TIP`
  **before** the five-path check. Ancestor stop is a command that
  exits 1 if false (`\|\| { echo …; exit 1; }`); do not run five-path
  yet. Two extra-path branches: not ancestor → merge `origin/main`
  (keep the `CODEOWNERS` deletion), do not `git rm` paths that exist
  on `origin/main` (live: #194 `renovate.json`); already ancestor →
  do not merge-for extra names, do not `git rm` a path that exists on
  `origin/main`, drop product-only extras from `$TIP` and re-run
  five-path. Do not run five-path on a merge-only tip.
- Then exact sorted equality: `git diff --name-only origin/main "$TIP"`
  vs the five paths (`CODEOWNERS` + three design docs +
  `docs/qa-onboarding.md`) against **then-current** `origin/main`.
- Status **line** carve-out vs `<accepted-sha>`;
  `docs/architecture.md` and `docs/README.md` byte-identical. Copied
  in place with `git checkout <accepted-sha> --` those three paths
  only.
- `Do: merge` is a **merge commit** of that `TIP` (repo
  `default_merge_style: merge`; `allow_squash_merge` and rebase are
  on — do **not** squash-merge or rebase-merge #195 onto `main`). No
  `force_merge`. Wait for `ci/woodpecker/pr/woodpecker`. Do not merge
  `fc1b20f`.
- Leftover tracker issue exists (`fj issue create`); #195 has no
  close-keyword immediately targeting that iid on any forbidden
  surface.

**Post-merge on `origin/main`** (tree contents only). After a successful
`Do: merge`, `origin/main` **is** the landed tip, so a five-path
`git diff --name-only origin/main` is **empty**. Do **not** require
that diff. Do **not** require descendant-of-`fc1b20f` after land
(pre-merge already required it on `head_commit_id`; post-merge audit
is tree contents, not ancestry):

- Four-path `ls-tree` empty on `origin/main`.
- This ADR, `docs/architecture.md`, and `docs/README.md` present; ADR
  Status **Accepted** (Status **line** carve-out only vs the accepted
  tree).
- `docs/qa-onboarding.md` lands all of D3: PR + Woodpecker
  `ci/woodpecker/pr/woodpecker` + no direct `main` + no deleting `main`;
  it does not contain `merge without approval`, `1 approving review`,
  `CODEOWNERS enforced`, `after approval`, `wait for the maintainer
  to review`, or `reviews and merges all MRs`; it **does** contain
  `practice, not \`required_approvals\``, `not a Forgejo approval
  gate`, and `maintainer merges as practice`; maintainer review is
  labeled **practice**; “never merge your own MRs” is practice, not
  `required_approvals`; “Stale reviews dismissed” is not kept as a
  protection row; QA `Fixes #N` teaching is kept; ADR 0001 and forge
  `docs/INVARIANTS.md` are pointed from the Branch Protection section.
- Vendor `.github/CODEOWNERS` under `packages/contracts-evm/lib/`
  unchanged.
- `.woodpecker.yaml` still defines the workflow named `woodpecker` at
  repo root so the required context stays `ci/woodpecker/pr/woodpecker`.
- No protection JSON change, no CAC reviewer dismiss, no deploy.

### Leftover-complete (follow-up issue; survives merge of #195)

- Dedicated plant-check `{n}` from post-delete `main`: both GET pairs
  satisfy Observability jq-able pass-iff (all six clauses). No
  mutation between pairs. No dismiss / `POST .../requested_reviewers`
  on `{n}`. Forbidden surfaces of `{n}` have no close-keyword
  immediately targeting the leftover tracker. Record `{n}`, **both**
  GET pairs (PR GET fields on each pair: `draft`, `title`,
  `changed_files`, `requested_reviewers`, `requested_reviewers_teams`,
  plus `updated_at` and `head.sha`; plus each pair’s reviews GET), with
  **pair 2** as the pass snapshot, proof that `updated_at` and
  `head.sha` did not change between the two PR GETs (auditor can tell
  pair 1 was taken, passed, and left unmutated across the 30s wait),
  four-path `ls-tree` on `main`, and `GET .../pulls/{n}` with
  `state == "closed"` and `merged == false`. `{n}` closed **without
  merge** (`fj pr close {n}`; no `-w` with leftover iid) **before**
  the leftover issue is closed. #195’s own official request does not
  count. “The next natural PR” does not count.

## Requirement-to-test mapping

See **Tests**. Implement records the commands run in the PR body.
Leftover plant-check JSON is recorded on the leftover tracker, not only
on issue #195.

## Cross-links

- [#195](https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/195)
- [#194](https://git.cl8y.com/code/cl8y-bridge-monorepo/pulls/194) (concurrent; may land first; not a freeze)
- [architecture.md](../architecture.md) (merge-gate pointer)
- [qa-onboarding.md](../qa-onboarding.md) (implement copy; D3)
- [cl8y-forgejo#48](https://git.cl8y.com/PlasticDigits/cl8y-forgejo/issues/48)
- [cl8y-forgejo INVARIANTS](https://git.cl8y.com/PlasticDigits/cl8y-forgejo/src/branch/main/docs/INVARIANTS.md)
- [cl8y-forgejo ADR 0003 item 8](https://git.cl8y.com/PlasticDigits/cl8y-forgejo/src/branch/main/docs/adr/0003-community-forge-and-woodpecker.md)
- [cl8y-agent-control#297](https://git.cl8y.com/PlasticDigits/cl8y-agent-control/issues/297) (authority)
- [cl8y-agent-control#429](https://git.cl8y.com/PlasticDigits/cl8y-agent-control/issues/429) (not a wait)
- [code/hello#15](https://git.cl8y.com/code/hello/pulls/15) (plant JSON / four-path names only; not a completed land)
