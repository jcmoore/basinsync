[github-coauthors]: <https://docs.github.com/en/pull-requests/how-tos/commit-changes/creating-a-commit-with-multiple-authors>
[github-issue-forms]: <https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms>

# Prompt requests

Contributors submit issues labeled `prompt-request` with a requested change,
acceptance criteria, constraints, and attribution preference. Maintainers discuss,
decline, or accept the request, then implement it with their own coding agents.
Submitting or editing an issue does not start an agent.

Start with manual handoff on the maintainer's device. This workflow needs no
hosted service or MCP server. The issue is the discussion record; the implementation
and its review remain the maintainer's responsibility.

## Prepare the fork

GitHub forks can have Issues disabled. Enable Issues on the test fork, create the
label, and merge the workflow files into its default branch before expecting the
*issue form*[~][github-issue-forms] to appear. For this experiment, the fork is
`jcmoore/basinsync` and its default branch is `sts-0`. Make these GitHub changes only
when ready to publish the experiment:

```sh
gh repo edit jcmoore/basinsync --enable-issues
gh label create prompt-request --repo jcmoore/basinsync --color 8250df \
  --description "Proposed change for a maintainer's coding agent"
```

The local helper requires Python 3.9+ and authenticated GitHub CLI. Start a new
Codex session in the checkout to discover the repo-local `prompt-request` skill.
Example invocation, replacing the URL with an actual issue on the fork:

```text
Use $prompt-request to implement https://github.com/jcmoore/basinsync/issues/NUMBER
and prepare a local PR draft for my review.
```

## Maintainer workflow

1. Review the issue and discussion. Comment to refine the request, close it as
   not planned when declining, or choose it for implementation. An explicit
   implementation instruction to your agent accepts a clear scope.
2. Capture the request with the helper, or let the skill do so:

   ```sh
   python3 scripts/prompt_request.py prepare https://github.com/jcmoore/basinsync/issues/NUMBER
   ```

3. Read the reported `request.md`, then record the accepted scope, source issue,
   snapshot SHA-256, and base commit in a neighboring `accepted.md`. The snapshot
   includes the issue and all comments. It records what was read, not proof of
   acceptance. Local snapshots are ignored by Git; include the digest and a scope
   summary in the eventual PR for a durable review record.
4. Work in a branch or worktree based on the intended target branch, implement
   the agreed criteria, and run the relevant checks. Inspect existing claims and
   linked PRs first; this prototype has no shared lock or atomic claiming.
5. Recheck before editing and before publishing:

   ```sh
   python3 scripts/prompt_request.py check .local/prompt-requests/CAPTURE/snapshot.json
   ```

   Changes to the issue or discussion require a fresh snapshot and reconciliation
   with the accepted scope. Fetching and checking are best-effort reads, not a
   GitHub transaction or a lock against edits after the check.
6. Prepare a PR description recording the resulting behavior, validation,
   accepted digest, full issue link, and chosen credit. Review it, then commit,
   push, and open the PR when those actions are authorized. Target the fork
   explicitly to avoid GitHub CLI's possible upstream default:

   ```sh
   gh pr create --repo jcmoore/basinsync --base sts-0 \
     --head BRANCH --title "Describe the implemented change" --body-file PR_BODY_FILE
   ```

The helper performs only GitHub GET requests and local writes. It never launches
an agent, changes labels, claims an issue, commits, pushes, or opens a PR. It checks
the supplied issue against the checkout's `origin`, rather than silently using
the fork's upstream. Closed issues, PRs, missing labels, incomplete discussions,
and ambiguous attribution fail with an actionable error. Existing captures are
never overwritten; use `--output` with a new directory for a second capture of
the same request.

## Attribution

The issue form defaults to acknowledging the requester's GitHub account. A PR
opened by the maintainer stays authored by the maintainer.

| Preference | Credit |
| --- | --- |
| Acknowledge my GitHub account | Credit the prompt contribution in the PR description. |
| Commit co-authorship (opt in) | Use the requester's supplied name and GitHub-associated email in a commit trailer, subject to project authorship policy. |
| No additional credit | Keep the source issue link without a personal acknowledgement. |

For *commit co-authorship*[~][github-coauthors], ask for the identity the requester
wants published; a GitHub-provided no-reply email can protect their personal
address. Never guess an email or promise contribution-graph credit. Preserve
co-author trailers in the final commit when squashing. A missing preference
produces no additional credit; malformed opt-in details require clarification.

The helper extracts the preference from the issue form. The maintainer or agent
must review any later clarification by the requester in the discussion. Choosing
no additional credit does not anonymize the issue or its author.

## Offline test

The bundled fixture is a synthetic issue and discussion; its URL and comment
identifiers are examples, not evidence of a live GitHub request. From the checkout:

```sh
python3 -m unittest discover -s tests -p 'test_prompt_request.py'
python3 scripts/prompt_request.py prepare https://github.com/jcmoore/basinsync/issues/1 \
  --fixture tests/fixtures/prompt-request.json --output .local/prompt-request-demo
python3 scripts/prompt_request.py check .local/prompt-request-demo/snapshot.json \
  --fixture tests/fixtures/prompt-request.json
```

Inspect the generated `request.md` and `attribution.txt`. A fixture check never
verifies GitHub state and cannot be mixed with a live snapshot. The tests exercise
stale discussion detection, attribution choices, fork isolation, complete comment
pagination, and non-overwriting captures without executing a coding agent.
