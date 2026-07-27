# Specs

This directory documents **what the bot does today**, feature by feature.

It began as a 1:1 capture of the pre-rewrite bot so the codebase could be rebuilt from the
ground up without losing behavior. That rewrite has landed, so these specs now describe the
shipped bot rather than the thing being replaced.

## Structure

- `specs/*.md` — **current-behavior specs.** Each describes a feature exactly as it works now.
- `specs/active/` — **specs for work in flight**: planned or in-progress features, describing
  *intended* behavior. A spec is promoted out of `active/` once it ships.
- `specs/architecture.md` — cross-cutting concerns shared by every command (shards, caching,
  database schema, rendering pipeline, scheduler, command framework).
- `specs/postgres.md`, `specs/rendering-placement.md`, `specs/scheduling.md` — cross-cutting
  subsystem specs, each shipped.
- `specs/qa-report.md` — the QA sweep of the pre-rewrite code. All findings are resolved; it's
  kept because its IDs (C-1, B-2, S-4 …) are cited from commits, specs, and code comments.

## Conventions for a spec

Each feature spec has:
- **Summary** — one line.
- **Command surface** — slash command name, options, permissions, autocomplete.
- **Behavior** — step-by-step of what happens on invocation.
- **External calls** — Foxhole API endpoints, Discord API, disk/DB.
- **Notes** — deliberate design decisions, and any remaining quirks worth knowing.
- **Acceptance criteria** — observable behavior an implementation must satisfy.

Where a spec explains *why* something is the way it is, that reasoning is usually a bug the
rewrite fixed. Those annotations are load-bearing: they're what stops the old behavior being
reintroduced as a "simplification".
