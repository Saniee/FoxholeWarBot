# Activity as tint intensity

**Status: option C tried and rejected; B and A not started, and not recommended.** Nothing of this
is in the code — the experiment was reverted once it had answered. See "What the experiment found".

Part 2 of the request that produced `specs/frontline-territory.md`, which shipped. That one decides
*which* colour a pixel gets; this decides *how strong* it is: a region where the war is actually
being fought washes darker, a quiet backline washes faint.

**Do not start this without looking at the shipped territory tint first.** It is the piece that was
actually asked for, it needed no new data, and it may well be enough on its own — a wash that
follows the front is a much bigger change to how the map reads than a wash that varies in strength.

## The one rule

The line still divides the tint exactly as `specs/frontline-territory.md` describes. **Activity
never moves the boundary, only the intensity either side of it.**

That separation is what makes this safe to want. Feeding activity into the field instead would change
where the line sits, and the centring took three rounds of live feedback to settle; feeding it into
`faction_tint_strength` cannot move the line by a pixel. Build it that way round even if a weighted
field looks tempting later.

## The problem: there are no player counts

**The War API publishes no player counts, per region or anywhere else.** The numbers on the
reference that look like activity — `470/hr`, `492/hr`, `220k 250k` — are casualties, and the
per-hour figures are rates that site derives by **polling over time and differencing**. The
underlying endpoint is one this bot already calls:

`GET /worldconquest/warReport/{map}` → `WarReport { total_enlistments, colonial_casualties,
warden_casualties, day_of_war, version }`, per region.

`totalEnlistments` is **not** a live player count, which is the first thing anyone checks it for. It
is a cumulative counter: a hex holds players in the low hundreds, and a real sample reads 22,656
against 470,272 casualties in the same region — about 21 deaths per enlistment, which is a ratio
accumulated over a war rather than a snapshot of one. The casualty fields in that same sample match
the reference screenshot's per-hex `220k 250k` pair exactly, which is what confirms those tiles are
war-report casualties and not something we cannot get.

**`totalEnlistments` is per-region — settled by two samples**, against a plausible-sounding argument
that it would be global (enlisting is joining a faction for the war, which is not something a player
does per hex). Two regions of the same war disagree, so the word was misleading and the data is not:
22,656 in a contested hex against 10,354 in Deadlands, with `version` differing too (135 against
101) while `dayOfWar` stays 500 across both. The response mixes scopes; enlistments and casualties
are on the per-region side of that line, `dayOfWar` on the other.

**But the same two samples argue against using enlistments as the intensity**, which is the opposite
of what being per-region first suggested:

| | enlistments | casualties | casualties per enlistment |
|---|---|---|---|
| contested hex | 22,656 | 470,272 | 20.8 |
| Deadlands, well behind the line | 10,354 | 44,468 | 4.3 |
| **ratio** | **2.2x** | **10.6x** | 4.8x |

Casualties separate a hot hex from a quiet one about five times more sharply than enlistments do.
For a signal whose whole job is to drive a *visible* difference in wash strength, that dynamic range
is the property that matters, and enlistments barely have it — a 2.2x spread across the sharpest
contrast on the map would map to a wash that looks flat. **Use casualties.**

Two cautions before that table gets treated as settled. These are cumulative figures, so they
describe the whole war rather than the present, and rates could well spread differently — the
comparison to make is between *rates*, once any history exists. And it is two regions, chosen as the
extremes; it says the spread exists, not what its distribution looks like across all 53.

It does cross-check nicely against our own render. Deadlands sits well behind the Colonial line in
the map we draw from this same war, the reference shows it at `0/hr`, and its cumulative casualties
are a tenth of the contested hex's. The activity signal really does track the front, which is the
assumption this rests on.

So what is actually available is:

- **Per-region cumulative casualties per faction, right now** — one fetch per region, already
  modelled in `api_definitions::foxhole`, already cached by `/war-report`. Free. Per-region
  cumulative enlistments come in the same response, at no extra cost, and are the weaker signal.
- **Rates** — only by keeping history. Two polls of every region and the difference between them,
  which is state this bot does not store today.

That distinction is the whole cost of this feature.

## What each option would actually cost

**A. Cumulative casualties as the intensity.** No new storage. Fetch 53 war reports on a full-map
render — the fan-out already exists for the dynamic half and this is the same shape.

The objection is that cumulative totals describe *the whole war*, not the present. A region fought
over for a week and quiet since keeps its number forever, so the map would darken over wherever the
war *has been* rather than where it is. By late war most of the front is dark and the picture stops
discriminating. **Usable as a first cut precisely because it is cheap and cannot mislead about
territory** — it is only ever intensity — but it answers a different question than the one asked.

**B. Casualty rate, by differencing polls.** The kind of number the reference actually shows, and
the one that means "where the war is right now". Casualties rather than enlistments on the evidence
above — both are per-region and free, and casualties carry roughly five times the contrast between a
contested hex and a quiet one. It needs history: a table of `(shard, region, timestamp,
colonial_casualties, warden_casualties)`, written by a background poll or on each render, read back
as a delta over a window.

That is a migration, a retention policy for rows nobody has asked to keep, and a `docs/tos.md` +
`docs/privacy.md` change in the same commit (`specs/docs-site.md`). It also makes the render depend
on the bot having been running for the length of the window: a fresh deployment draws a flat map for
its first hours, and that needs a defined fallback rather than being discovered in a screenshot.

**C. Structure density as a stand-in.** Free, already in hand, needs nothing new — the footings
count per region is a fair proxy for "how much is going on here", and it is data the render already
holds. Not the same quantity as activity, and it would make heavily-built quiet regions dark. Worth
one experiment before paying for B, because the experiment costs an afternoon and the answer might
be "close enough".

**Built, measured, and the answer is no.** See "What the experiment found" below.

## What the experiment found

Option C was built as far as a render and no further: a second value per cell on the influence grid,
carrying each region's footing count normalized against the busiest, interpolated per pixel by the
same pass that reads `F` and multiplied into the wash strength. Run against a live Able shard, it
fails on both halves at once.

**The proxy does not track the front.** The ranking is the finding, and it needs no interpretation:

| | reading |
|---|---|
| Farranac Coast — Warden backline | **1.000** |
| Oarbreaker Isles — islands, far rear | 0.778 |
| Callahans Passage — *the line runs through it* | 0.722 |
| Deadlands — the spec's own example of a quiet hex | 0.667 |
| Marban Hollow — *the line runs through it* | 0.611 |
| Pari Peak — far rear | 0.333 |

Both contested hexes sit mid-table, below regions deep behind either line. This is not a weak
correlation with the front, it is no correlation: what the count actually measures is how many
separate built-up places a region has, which is mostly a fact about its terrain and its size.

**And the spread is too small to see anyway.** 0.333 to 1.000 is about 3x, and clustered — most of
the map falls between 0.44 and 0.78. That is the *enlistments* figure this spec already rejected for
being flat, which is a nice consistency check on the reasoning above it. At the 0.6 floor the
argument below calls for, the rendered map is near-indistinguishable from one with the feature off.

**The part that matters for B.** Turning the floor down to 0.15 does make the variation plainly
visible — the mechanism works — and it also turns Pari Peak, Olavis Wake and Kuura Strand nearly
colourless. So the squeeze is real and it belongs to the *rendering*, not to the input: visible
variation costs backline legibility, and the floor is the only dial between them. B is not refuted
by this — casualties carry 10.6x against footings' 3x, so a real signal plausibly clears the bar at a
safe floor. But "plausibly" is the whole of the evidence, and B costs a migration, a retention
policy, a docs change and a fresh-deployment fallback to find out.

**Recommendation: stop here.** The territory tint already answers the question that was asked, and
the drawn line already marks where the fighting is. Reopen this only if a live map is looked at and
the flat wash is what is missing from it.

**The apparatus was reverted with the result.** Keeping it was tempting — all three options are the
same rendering change downstream of a different number, so B would rebuild it — but it was dead by
construction, wired to no command and no setting, and a rejected experiment left in the tree is
something every later reader has to work out the status of. This section is the durable part; the
code was half a day and is described well enough here to redo.

What it took, if it is ever redone: a second `Vec<f32>` on `Field` filled at the same grid points by
normalized inverse-distance from the per-region figures (`Σ v/(d²+r²) / Σ 1/(d²+r²)`, with `r` about
half a region across — plain IDW makes every hex centre a bullseye), read back by the same bilinear
walk that reads `F`, and multiplied into the wash strength as `floor + (1-floor) × activity`. The
two tests worth writing again are the ones that hold the one rule: assert the traced contour is
*equal* before and after, and walk between two readings bounding the largest single step against the
total, so the intensity can never come back as a hex lattice.

## The thing to get right: this reintroduces hex edges

The territory tint's whole point is that the wash stops being hex-shaped. **A per-region intensity
puts hex boundaries straight back into it** — not as colour flips this time, but as steps in
strength, which on a large flat wash are just as visible.

So the intensity has to be a *field*, not a lookup:

- Sample the per-region figure at each hex centre, then interpolate across the map the way the
  influence field is interpolated — so a region twice as busy as its neighbour reads as a gradient
  between them rather than a visible hexagon.
- The influence field's own grid is the obvious carrier: it already spans the world in world space,
  it is already interpolated per pixel by `Field::along`, and a second value per cell costs one more
  `Vec<f32>` and no extra sampling passes.
- Clamp the mapped strength to a range with a floor well above zero. A quiet region must still read
  as *held*, or the territory tint's answer disappears wherever nobody is fighting — which is most
  of the map, most of the time.

## Order, if it is wanted

**C to find out whether the effect is worth having, B to do it properly, A only if cheapness beats
accuracy.** All three are the same rendering change downstream of a different number, so the
experiment is not wasted whichever way it goes.

C was done on those terms and came back negative. The sentence above is still true and is why the
rendering recipe is written down rather than kept as code: a better number would reuse the shape, not
the lines.
