# Future Plans

This document records ideas that are intentionally deferred until usage data shows they are
worth building. These are not active implementation specifications.

## Bot Dashboard

Eventually provide a complete web dashboard for managing the bot, including guild settings,
scheduled reports, approvals, and usage statistics. The dashboard should become a more convenient
management surface than slash commands, not a replacement for the bot's Discord functionality.

### Trigger

Do not start this work until the bot's usage has revived enough to justify operating and securing
another application surface. `/usage-stats` is the current measure for that decision.

### Deployment Direction

Run the dashboard as a separate service alongside the bot in Docker Compose:

```text
reverse proxy
      |
 dashboard ----\
                +---- Postgres
 bot -----------/
```

The dashboard should use the existing Postgres database over the private Compose network. Postgres
must not be exposed publicly, and the web service should not be embedded in the bot process.

### Design Constraints

- Use Discord OAuth2 for login and verify the user's Discord permissions before allowing guild changes.
- Keep authentication, authorization, session handling, CSRF protection, and HTTPS behind deliberate
  design decisions rather than treating the dashboard as an internal tool.
- Reuse the bot's validation and application rules for settings and schedule changes; do not let the
  web layer write arbitrary database rows.
- Review Postgres connection-pool sizing because the bot and dashboard will share the database.
- Decide whether the first version is read-only or supports settings and schedule mutations.
- Consider a reverse proxy for TLS termination and public routing.

### Initial Scope Candidates

- View and edit guild settings.
- View, create, edit, and remove scheduled reports.
- Review full-map schedule requests.
- View usage statistics and trends.
- Inspect the current war and bot status.
