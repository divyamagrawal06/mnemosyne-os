# MnemosyneOS local dashboard

## Scene and theme

A student switches from slides to a laptop browser projected in a lit classroom. A light paper surface and dark readable text match the deck and remain legible under projection.

## Visual language

Use a restrained warm neutral canvas, an off-white work surface, graphite text, and a forest-green action color. Muted copper identifies saved evidence and pending research; failures use a separate red with a text label. CSS tokens use OKLCH. No decorative gradients, glass panels, or large metric-card grid.

## Typography

Segoe UI and system sans-serif for the app; Consolas and system monospace for commands, hashes, and raw evidence. Heading scale 20/26/34 px, body 14-16 px, secondary labels at least 12 px. Prose stays within 72 characters per line.

## Layout and components

A slim persistent sidebar holds five views: Overview, Kernel, Replay, Tests, and Project. A top utility bar identifies local connection and links presentation materials. The main work area has a page heading, evidence-source switch, provenance strip, and a single primary run action. A horizontal three-step sequence anchors the overview. Tables, definition lists, split inspectors, and an expandable console organize evidence.

Use ordinary buttons, selects, links, details, and tables. Status is always expressed in text. Focus rings are visible. Narrow screens turn the sidebar into a compact navigation row and stack the two-column inspector. Tables scroll inside their region, never the whole page. Transitions are limited to color and opacity and respect prefers-reduced-motion.

## Evidence display

Saved demonstration evidence is labeled with its capture date and code baseline. Live results use the actual run identifier, timing, stage status, and working-tree revision. A failed or unstarted step shows no inherited success metrics. An absent event trace is described as unavailable rather than synthesized. The UI runs on the host and does not imply a guest desktop exists.
