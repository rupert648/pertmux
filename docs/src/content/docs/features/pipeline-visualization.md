---
title: Pipeline Visualization
description: Compact CI/CD pipeline status dots for at-a-glance pipeline health.
---

pertmux renders CI/CD pipeline status as compact colored dots, inspired by [glim](https://github.com/junkdog/glim). Each job is a single dot, grouped by stage, giving you an instant overview of pipeline health.

## Status colors

| Color | Status |
|-------|--------|
| Green | Success |
| Red | Failed |
| Orange | Running |
| Yellow | Pending / Preparing / Waiting |
| Purple | Manual |
| Dark gray | Canceled / Skipped |
| Gray | Created |

## Failed jobs

When jobs fail, pertmux shows the failed job names below the dots:

```
jobs     ●●●●● ●●● ●●
         ✗ lint, unit-tests
```

Jobs with `allow_failure` set are excluded from the failure list.

## Forge support

Pipeline visualization works with both GitLab and GitHub:

- **GitLab**: Fetches pipeline jobs from the head pipeline
- **GitHub**: Fetches check runs from the head SHA and converts them to the same visual format
