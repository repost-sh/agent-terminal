# Issue tracker: Linear

Issues and specs live in Linear, team **46h**, project **agent-terminal**. Use the Linear MCP tools (`mcp__plugin_linear_linear__*`). Code and PRs live on GitHub (repost-sh/agent-terminal); GitHub Issues are not used.

## Conventions

- Create / update an issue: `save_issue` with `team: 46h`, `project: agent-terminal`, and a `milestone` (Core CLI, Evidence, Dashboard, Ship).
- Read an issue: `get_issue` (body, labels, relations); comments with `list_comments`.
- List issues: `list_issues` filtered by project, label, state or milestone.
- Sub-issue of a parent: `parentId` on `save_issue`.
- Blocking: native relations via `blockedBy` / `blocks` on `save_issue`. A ticket is unblocked when every blocker is Done.
- Comment: `save_comment`.
- Labels: `addLabels` / `removeLabels` on `save_issue`.
- Close: set `state` to Done (or Canceled) with a closing comment.
- Branches: use the issue's `gitBranchName`; PR titles/bodies reference the issue id (e.g. 46H-20) so Linear links them.

## When a skill says "publish to the issue tracker"

Create a Linear issue in team 46h, project agent-terminal.

## When a skill says "fetch the relevant ticket"

`get_issue` by id (e.g. 46H-20).

## Wayfinding operations

- Map: one issue labelled `wayfinder:map`; child tickets are its sub-issues, labelled `wayfinder:<type>`.
- Blocking: native "blocked by" relations.
- Frontier: open sub-issues of the map with no open blockers and no assignee; first in sort order wins.
- Claim: assign to "me". Resolve: comment the answer, set Done, add a pointer to the map's Decisions-so-far.
