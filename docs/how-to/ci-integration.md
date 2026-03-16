# How to Integrate buildfix with CI/CD

This guide shows common patterns for running buildfix in automated pipelines.

## Overview

buildfix fits into CI/CD in two modes:

1. **Plan-only**: Generate plans on PRs for review (informational)
2. **Plan + Apply**: Automatically fix issues on the main branch

## Exit Codes

buildfix uses semantic exit codes for CI integration:

| Code | Meaning | CI Action |
|------|---------|-----------|
| 0 | Success | Continue |
| 1 | Tool error | Fail the job |
| 2 | Policy block | Configurable (fail or warn) |

## GitHub Actions

### Plan-Only on Pull Requests

Generate a plan and upload as artifact:

```yaml
name: buildfix-plan

on:
  pull_request:
    branches: [main]

jobs:
  plan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run sensors
        run: |
          # Run your sensors first
          cargo run -p builddiag
          cargo run -p depguard

      - name: Generate buildfix plan
        run: cargo run -p buildfix -- plan

      - name: Upload plan artifacts
        uses: actions/upload-artifact@v4
        with:
          name: buildfix-plan
          path: artifacts/buildfix/

      - name: Comment patch preview
        if: hashFiles('artifacts/buildfix/patch.diff') != ''
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const patch = fs.readFileSync('artifacts/buildfix/patch.diff', 'utf8');
            if (patch.trim()) {
              github.rest.issues.createComment({
                issue_number: context.issue.number,
                owner: context.repo.owner,
                repo: context.repo.repo,
                body: `## buildfix Plan\n\n\`\`\`diff\n${patch}\n\`\`\``
              });
            }
```

### Apply on Main Branch

Automatically apply safe fixes after merge:

```yaml
name: buildfix-apply

on:
  push:
    branches: [main]

jobs:
  apply:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ secrets.BUILDFIX_TOKEN }}  # PAT for push

      - name: Run sensors
        run: |
          cargo run -p builddiag
          cargo run -p depguard

      - name: Plan and apply
        run: |
          cargo run -p buildfix -- plan
          cargo run -p buildfix -- apply --apply

      - name: Commit changes
        run: |
          git config user.name "buildfix[bot]"
          git config user.email "buildfix@example.com"
          git add -A
          git diff --cached --quiet || git commit -m "chore: apply buildfix repairs"
          git push
```

## GitLab CI

### Plan Stage

```yaml
buildfix:plan:
  stage: analyze
  script:
    - cargo run -p builddiag
    - cargo run -p depguard
    - cargo run -p buildfix -- plan
  artifacts:
    paths:
      - artifacts/buildfix/
    expire_in: 1 week
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

### Apply Stage (Main Only)

```yaml
buildfix:apply:
  stage: fix
  script:
    - cargo run -p builddiag
    - cargo run -p depguard
    - cargo run -p buildfix -- plan
    - cargo run -p buildfix -- apply --apply
    - |
      git config user.name "buildfix"
      git config user.email "buildfix@example.com"
      git add -A
      git diff --cached --quiet || git commit -m "chore: apply buildfix repairs"
      git push https://oauth2:${GITLAB_TOKEN}@${CI_SERVER_HOST}/${CI_PROJECT_PATH}.git HEAD:${CI_COMMIT_REF_NAME}
  rules:
    - if: $CI_COMMIT_BRANCH == "main"
```

## Handling Policy Blocks

Exit code 2 (policy block) can indicate:

- Fixes denied by policy
- Precondition mismatch (repo drift)
- Dirty working tree
- Caps exceeded

For informational runs, treat exit 2 as success:

```yaml
- name: Generate plan (allow blocks)
  run: cargo run -p buildfix -- plan || test $? -eq 2
```

For enforcement, let exit 2 fail the job.

## Preserving Artifacts

Always upload buildfix artifacts for debugging:

```yaml
- uses: actions/upload-artifact@v4
  if: always()
  with:
    name: buildfix-artifacts
    path: |
      artifacts/buildfix/plan.json
      artifacts/buildfix/plan.md
      artifacts/buildfix/patch.diff
      artifacts/buildfix/apply.json
      artifacts/buildfix/apply.md
```

## Preventing Loops

When buildfix commits changes, prevent infinite CI loops:

1. **Skip CI in commit message**:
   ```bash
   git commit -m "chore: apply buildfix repairs [skip ci]"
   ```

2. **Check for buildfix commits**:
   ```yaml
   - if: "!contains(github.event.head_commit.message, 'buildfix')"
   ```

3. **Use a bot account** with limited permissions

## Cockpit Integration

buildfix produces `report.json` compatible with the cockpit receipt format. Configure your cockpit to consume:

```
artifacts/buildfix/report.json
```

This provides visibility into:
- Fix plan availability
- Apply status
- Blocked fixes and reasons

## See Also

- [CLI Reference](../reference/cli.md)
- [Exit Codes](../reference/exit-codes.md)
- [Output Schemas](../reference/schemas.md)
