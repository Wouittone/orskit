# Bot publishing with human reviews

Keep the Copilot app signed in as your human account. Publish agent branches and
create their PRs with the existing `orskit-copilot` GitHub App instead. This
separates the push actor and PR creator from the human reviewer; changing Git
author metadata alone does not do that.

## One-time setup

Install PowerShell 7.2 or later and Git. Install the `orskit-copilot` App on
`Wouittone/orskit`, with Contents and Pull requests write permissions.
Save its RSA private-key PEM outside all repository checkouts, by default at:

```text
C:\Users\<you>\.config\orskit-copilot\orskit-copilot.pem
```

Restrict access to the key to your Windows account and trusted administrators.
Do not paste it into chat, check it into Git, or store it in the app's repository
configuration. Use `-PrivateKeyPath` if it is stored elsewhere.

From the checkout, verify authentication without pushing or creating a PR:

```powershell
pwsh -NoProfile -File .\scripts\publish_bot.ps1 -Check
```

This creates a repository-scoped, short-lived installation token, verifies the
App and repository access, and revokes the token before returning. It does not
modify the app login, `GH_TOKEN`, credential manager, Git author configuration,
or remotes.

## Publish

Work on a non-`main` branch and commit only when the user authorizes it. The
publisher requires a clean working tree and never creates branches or commits.
Prepare a PR body outside the checkout using `.github\pull_request_template.md`.
Keep its headings, comments, checklist items, and ordering; include exact
validation commands and their results.

```powershell
pwsh -NoProfile -File .\scripts\publish_bot.ps1 `
    -Base main -Title "Describe the change" `
    -BodyFile C:\path\to\completed-pr-body.md
```

Use the intended base explicitly; for a stacked PR, pass its parent branch.
Add `-Draft` for a new draft. To update a branch with an existing bot-created PR:

```powershell
pwsh -NoProfile -File .\scripts\publish_bot.ps1 -Base main
```

This preserves the PR title, body, draft status, and target. It rejects a
human-created PR rather than pretending bot-pushed commits change its creator.
For branch-only publication, use `-PushOnly -Base main`.

The script uses a fresh token for both Git push and REST PR creation. It never
force-pushes, bypasses branch protections, changes upstream tracking, or falls
back to personal credentials. Failed PR creation may leave an already-pushed
branch: correct the reported error and rerun the same command. Open bot PRs are
reused, so successful reruns do not create duplicate PRs. Workflow-file pushes
are intentionally excluded from the token's permissions; those need a separate
explicitly authorized publishing path.

## Review in the app

Open the printed PR URL through **My work** in the Copilot app and submit reviews
as your human account. The app's native **Create PR** button still uses its own
selected account: do not use that button for bot publication. Shell-created PRs
are not automatically attached to the originating app session.

The publisher authenticates as your custom `orskit-copilot[bot]`, not GitHub's
managed Copilot cloud agent. Commit authorship remains unchanged; retain the
repository's Copilot co-author trailer for agent-assisted commits.

## Maintainer checks

Run the offline regression scenarios without any installed App credentials or
network access:

```powershell
pwsh -NoProfile -File .\scripts\test_publish_bot.ps1
```

The scenarios verify JWT signing, scoped credentials, PR creation/reuse, unsafe
publication refusals, token revocation, and process-environment restoration.

See GitHub's [installation authentication documentation](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-as-a-github-app-installation).
