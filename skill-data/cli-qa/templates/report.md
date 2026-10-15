# CLI QA report: {tool}

- Date: {date}
- Version: {tool --version}
- Shell: {zsh, bash or fish}
- Sizes tested: {120x40, 80x24}
- Findings: {n} ({critical} critical, {major} major, {minor} minor)

## Findings

### F1. {one-line summary}

- Severity: critical | major | minor
- Area: help | version | exit codes | errors | prompts | pipes | width | slow work
- Command: `{the exact command}`
- Exit code: {code} (command {id} in `history`)
- Expected: {what should happen}
- Actual: {what happened, with the lines that show it from `history show {id}`}
- Screenshot: `screenshots/f1.png` (when the problem is visual)

## What worked

{Short list of the checks that passed, so the reader knows what was covered.}
