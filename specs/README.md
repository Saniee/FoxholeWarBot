# Specs

This directory captures **what the bot does today**, feature by feature, so the codebase can
be rewritten from the ground up while preserving behavior 1:1.

## Structure

- `specs/*.md` — **current-behavior specs.** Each describes an existing feature exactly as it
  works now (including quirks and known bugs, which are called out explicitly). These are the
  reference for a faithful reproduction.
- `specs/active/` — **specs for work in flight.** When a feature needs an overhaul rather than a
  straight port, its spec is promoted here and rewritten to describe the *intended* behavior.
- `specs/architecture.md` — cross-cutting concerns shared by every command (shards, caching,
  database schema, rendering pipeline, scheduler).
- `specs/qa-report.md` — a QA sweep: crash-causing bugs, correctness bugs, and reliability
  issues found in the current code, ranked by severity. Read this before porting.

## Conventions for a spec

Each feature spec has:
- **Summary** — one line.
- **Command surface** — slash command name, options, permissions, autocomplete.
- **Behavior** — step-by-step of what happens on invocation.
- **External calls** — Foxhole API endpoints, Discord API, disk/DB.
- **Quirks & known bugs** — behavior a rewrite must decide to keep or fix (cross-linked to
  `qa-report.md`).
- **Acceptance criteria** — observable behavior a reproduction must satisfy.

Status legend used in headings: ✅ port as-is · ⚠️ port but fix noted bugs · 🔧 needs overhaul
(promoted to `active/`).
