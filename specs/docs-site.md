# Docs site (`docs/`)

## Summary
The GitHub Pages site at <https://saniee.github.io/FoxholeWarBot/> — landing page, Terms of
Service, Privacy Policy and FAQ. Four Jekyll pages on the `pages-themes/midnight` remote theme,
deployed by `.github/workflows/pages.yml`.

## Pages
| File | Purpose |
|---|---|
| `index.md` | One-line pitch, invite link, links to the other three, attribution notice. |
| `tos.md` | What the bot does, the field-by-field list of what it stores, retention, availability, attribution, contact. |
| `privacy.md` | Collected / not collected, command arguments, the API cache, removal. |
| `faq.md` | Required permissions, getting started, the command list, schedule phrases, downtime. |

Every page needs the `layout: default` front matter. `faq.md` shipped without it for a long time
and rendered unthemed.

## The invariant
**Nothing on the site may describe data the bot doesn't store or a permission it doesn't use.**
An inaccurate privacy policy is worse than a sparse one, and this is the thing that rots: the
pages are edited when features are planned, not when they ship.

Concretely, the "what it stores" list in `tos.md` and `privacy.md` is the `guilds` and `cronjobs`
tables spelled out in prose. Changing `migrations/` means changing both pages in the same commit.

Wrong claims that were removed in this pass, kept here so they don't come back:
- the guild **owner's** ID is stored — it never was, and no owner is read anywhere;
- the bot **DMs a server owner** about errors — no such code path has ever existed;
- **Add Reactions** and **Send Messages in Threads** are required — the bot runs on the `GUILDS`
  intent alone, never reacts, and posts only slash replies and webhook messages.

## Permissions, as documented
View Channel, Send Messages, Embed Links, Attach Files, Use Application Commands, and Manage
Webhooks — the last needed only for `/schedule-report` and `/remove-report`, which are themselves
gated behind `MANAGE_WEBHOOKS` on the member side.

## Contact details
The support invite appears in `docs/`, `README.md` and `commands::common::SUPPORT_INVITE`. All
three use the `discord.gg/9wzppSgXdQ` form so the link in Discord and the link on the site are
visibly identical. The dead Twitter link is gone; Discord and email remain.

## Schedule phrases
Listed in `faq.md` and, identically, inline in `/schedule-help`. The old `prnt.sc` screenshot
link is gone from both — it was one link-rot away from being the only documentation (QA L-1).

## Deployment
`.github/workflows/pages.yml` builds `./docs` with `actions/jekyll-build-pages` and deploys on
push to `master` touching `docs/**` or the workflow itself, plus `workflow_dispatch`. Requires
Settings → Pages → Source = "GitHub Actions".

## Out of scope
- Monetization, donation or sponsorship language. The full-map gate is an approval form, not a
  paid tier (`specs/active/premium-full-map.md`), so no payment wording belongs anywhere here.
- Legal review. These are plain-language docs for a free community tool.

## Acceptance criteria
- No statement in `docs/` describes data the bot doesn't collect or a permission it doesn't use.
- The stored-data list matches the Postgres schema field for field.
- A Siege Camp attribution / non-affiliation notice is present.
- Support contact details are current and consistent between the docs and in-bot messages.
- Every page carries `layout: default`.
