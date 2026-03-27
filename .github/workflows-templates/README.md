# buildfix CI Workflow Templates

This directory contains GitHub Actions workflow templates for integrating buildfix into your CI/CD pipeline. These templates are designed to be copied and customized for your specific needs.

## Overview

| Template | Purpose | Trigger | Auto-Apply |
|----------|---------|---------|------------|
| [`pr-lane.yml`](./pr-lane.yml) | PR validation | Pull requests | No (plan-only) |
| [`main-lane.yml`](./main-lane.yml) | Main branch maintenance | Push to main | Yes (safe fixes) |
| [`scheduled-audit.yml`](./scheduled-audit.yml) | Periodic security audit | Schedule/Manual | No (plan-only) |

## Quick Start

### 1. Copy Templates

```bash
# Copy templates to your workflow directory
cp .github/workflows-templates/*.yml .github/workflows/
```

### 2. Customize for Your Project

Edit each workflow file to:
- Enable/disable sensors (cargo-machete, cargo-deny, etc.)
- Adjust schedules for scheduled-audit
- Configure bot credentials for auto-commits
- Set up notifications

### 3. Configure Permissions

Ensure your repository has the required permissions:

| Workflow | Permissions Needed |
|----------|-------------------|
| pr-lane | `pull-requests: write`, `contents: read` |
| main-lane | `contents: write` |
| scheduled-audit | `issues: write`, `contents: read` |

## Workflow Details

### PR Lane (`pr-lane.yml`)

Runs on every pull request and generates a repair plan without applying changes.

**Features:**
- Plan-only mode (no changes applied)
- Uploads plan artifacts (plan.json, plan.md, patch.diff)
- Optional PR comment with plan summary
- Proper exit code handling

**Customization Points:**
```yaml
# Enable/disable sensors
- name: Run cargo-machete
  continue-on-error: true  # Set to false to fail on issues
  
# Configure PR comments
- name: Post PR comment
  if: always() && github.event_name == 'pull_request'
```

**Exit Code Handling:**
```yaml
# Fail on policy blocks (optional)
- name: Check plan status
  if: always()
  run: |
    if [ "$EXIT_CODE" -eq 2 ]; then
      # Uncomment to fail on policy blocks
      # exit 2
    fi
```

### Main Lane (`main-lane.yml`)

Runs on pushes to main/master branch and auto-applies safe fixes.

**Features:**
- Auto-applies safe fixes
- Optional bot commits with `[skip ci]` to prevent loops
- Ignores changes to Cargo.toml/Cargo.lock to prevent CI loops
- Separate job for guarded fixes (requires manual trigger)

**CI Loop Prevention:**
```yaml
on:
  push:
    branches: [main, master]
    paths-ignore:
      - '**/Cargo.toml'
      - '**/Cargo.lock'
      - '.github/workflows/main-lane.yml'
```

**Bot Commit Configuration:**
```yaml
env:
  GIT_AUTHOR_NAME: buildfix-bot
  GIT_AUTHOR_EMAIL: buildfix-bot@users.noreply.github.com
  GIT_COMMITTER_NAME: buildfix-bot
  GIT_COMMITTER_EMAIL: buildfix-bot@users.noreply.github.com
```

**Guarded Fixes:**
Guarded fixes require manual workflow dispatch:
1. Go to Actions → buildfix Main Lane
2. Click "Run workflow"
3. The `guarded-apply` job will run with `--allow-guarded`

### Scheduled Audit (`scheduled-audit.yml`)

Runs periodic security and dependency audits.

**Features:**
- Runs cargo-audit, cargo-deny, cargo-machete, cargo-outdated
- Creates GitHub issues for findings
- Generates comprehensive repair plan
- 90-day artifact retention

**Schedule Configuration:**
```yaml
on:
  schedule:
    # Weekly on Sunday at midnight UTC
    - cron: '0 0 * * 0'
    
    # Other examples:
    # - cron: '0 6 * * 1'  # Monday 6 AM UTC
    # - cron: '0 0 1 * *'  # Monthly on 1st
```

**Issue Creation:**
The workflow automatically creates or updates a GitHub issue with the audit results when issues are found.

## Required Secrets

| Secret | Required For | Description |
|--------|--------------|-------------|
| `GITHUB_TOKEN` | All workflows | Default token (automatically provided) |
| `BUILDFIX_BOT_TOKEN` | main-lane | Personal access token for bot commits (optional) |

### Creating a Bot Token (Optional)

For main-lane auto-commits, you may want to use a dedicated bot token:

1. Create a GitHub App or Personal Access Token
2. Grant `contents: write` permission
3. Add as repository secret: `BUILDFIX_BOT_TOKEN`

## Exit Codes

buildfix uses specific exit codes to communicate results:

| Exit Code | Meaning | Action |
|-----------|---------|--------|
| 0 | Success | No issues or plan generated successfully |
| 2 | Policy block | Preconditions not met or denied fix - review manually |
| 1 | Error | Tool error - check logs |

### Handling Exit Codes in Workflows

```yaml
- name: Generate repair plan
  id: plan
  run: |
    cargo run -p buildfix -- plan --artifacts-dir artifacts --out-dir output
    EXIT_CODE=$?
    echo "exit_code=$EXIT_CODE" >> $GITHUB_OUTPUT

- name: Handle policy blocks
  if: steps.plan.outputs.exit_code == '2'
  run: |
    echo "::warning::Policy block - review plan manually"
    # Optionally fail the workflow:
    # exit 2
```

## Customization Guide

### Adding Sensors

Add new sensor steps before the plan generation:

```yaml
- name: Run custom tool
  continue-on-error: true
  run: |
    mkdir -p artifacts/custom-tool
    your-tool --format json > artifacts/custom-tool/report.json || true
```

### Filtering Files

Limit which files are processed:

```yaml
on:
  push:
    paths:
      - '**/Cargo.toml'
      - '**/Cargo.lock'
      - 'src/**'
```

### Conditional Execution

Run workflows only under certain conditions:

```yaml
jobs:
  plan:
    if: github.event.pull_request.draft == false
    # ...
```

### Notifications

Add Slack or email notifications:

```yaml
- name: Notify on failure
  if: failure()
  uses: slackapi/slack-github-action@v1
  with:
    payload: |
      {
        "text": "buildfix failed in ${{ github.repository }}"
      }
  env:
    SLACK_WEBHOOK_URL: ${{ secrets.SLACK_WEBHOOK }}
```

## Best Practices

1. **Start with PR Lane**: Begin with plan-only mode to understand what changes buildfix will make.

2. **Review Plans**: Always review generated plans before enabling auto-apply.

3. **Use Bot Tokens**: For main-lane, use a dedicated bot token to distinguish automated commits.

4. **Configure CI Loops**: Ensure `paths-ignore` is configured to prevent infinite CI loops.

5. **Set Retention**: Adjust artifact retention based on your needs (default: 30-90 days).

6. **Monitor Exit Codes**: Handle exit code 2 (policy blocks) appropriately for your workflow.

7. **Schedule Audits**: Run scheduled audits during low-traffic periods.

## Troubleshooting

### Workflow Not Triggering

- Check branch names match your repository (main vs master)
- Verify workflow file is in `.github/workflows/`
- Ensure `paths-ignore` patterns aren't too broad

### Bot Commits Failing

- Verify `BUILDFIX_BOT_TOKEN` secret is set correctly
- Check token has `contents: write` permission
- Ensure token hasn't expired

### CI Loops

- Verify `paths-ignore` includes Cargo files
- Check commits include `[skip ci]` in message
- Ensure workflow triggers don't overlap

### Missing Artifacts

- Check `upload-artifact` step ran successfully
- Verify artifact names are unique per run
- Check retention days setting

## Contributing

To contribute improvements to these templates:

1. Edit the templates in `.github/workflows-templates/`
2. Test in a sample repository
3. Submit a pull request with description of changes

## License

These templates are part of buildfix and are licensed under MIT OR Apache-2.0.
