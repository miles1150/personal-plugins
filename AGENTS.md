# UI and interaction design

Avoid unnecessary secondary labels and descriptions. Let the layout do the explaining.

Keep explanations that affect a user's decision: permission scope, destructive effects, recovery actions, and meaningful limitations. Move implementation details and prototype simulation controls out of normal product flows.

## Frontend design principles

Apply the following when creating or reviewing UI, alongside the project's existing design system:

- Ground the layout, typography, color, and copy in the user's actual task. Preserve an explicitly approved visual direction; do not import a generic visual theme just because a skill uses it as an example.
- Remove redundant labels above headings or content. Do not use all-caps labels or letter-spaced eyebrow text as decoration; preserve legitimate product names and technical acronyms when needed.
- Keep a headline's treatment coherent. Do not emphasize one arbitrary word with a different color, weight, or italic style.
- Visual structure is information. Borders, dividers, grouping, labels, and subheadings must communicate a real relationship, boundary, state, or hierarchy. Use spacing and alignment when they communicate the same thing more simply.
- Use numbered markers only for an actual sequence, ordered procedure, or timeline. Do not number unrelated features or cards.
- Avoid repeating the same explanation in the page subtitle, card, badge, and footer. Each visible piece of copy should have one useful job.
- Use plain user-facing names and sentence case. Buttons name their action, and confirmation messages use the same vocabulary. Empty and error states provide a concrete next step.
- Avoid turning every item into an identical bordered card. Choose lists, tables, groups, or cards according to the information and interaction.
- Use motion sparingly to explain a change caused by the user; avoid decorative entrance animations across every section. Respect reduced-motion preferences.
- Preserve visible keyboard focus, accessible contrast, adequate interaction targets, and mobile layouts. Simplifying copy does not justify removing meaningful labels or permission disclosures.
- Before building, state a brief task-specific layout and token plan. Review it against the brief, then inspect the resulting screens and remove decoration or copy that does not help the task.
- Keep prose comfortably readable (normally under 80 characters per line); define a deliberate type scale instead of adding fonts or emphasis to manufacture hierarchy.
- Give the primary task visual priority. Avoid competing focal points, automatic arrow suffixes, monospace metadata, or repeated decorative separators unless they convey something useful in this product.
- Review real rendered screens at desktop and mobile sizes, including empty, error, pending, and permission states. A source-code scan or a prototype check alone is not evidence that the deployed interface works.
- Keep CSS specificity predictable; resolve conflicting spacing or component rules rather than layering overrides until a screenshot happens to look right.

Source reviewed on 2026-10-31: [Anthropic frontend-design skill](https://github.com/anthropics/skills/blob/HEAD/skills/frontend-design/SKILL.md). The user's explicit instructions and approved product model take precedence over generic stylistic defaults.
