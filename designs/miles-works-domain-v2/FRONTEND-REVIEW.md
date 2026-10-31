# Frontend design review

Reviewed against the full [official Anthropic frontend-design skill](https://github.com/anthropics/skills/blob/HEAD/skills/frontend-design/SKILL.md), the approved Miles Works model, and root `AGENTS.md`.

## Scope

This review covers the working-tree dashboard and account HTML, CSS, and rendering code. The earlier prototype review remains separate. Neither source inspection nor mock browser tests establish that production has been updated.

## Findings

- Current page headings use a consistent treatment; no arbitrary word receives a special color, weight, or italic treatment.
- Dashboard and account screens do not need decorative eyebrows or feature numbering. Product names and genuine acronyms are not decorative all-caps labels.
- The assistant list uses rows. Permission, session, and client sections represent different relationships, so their grouping is meaningful; further nested cards would not help.
- Session rows exposed `host_kind`, `status`, and `capability` as a metadata string. The UI revision separates source, receiving capability, and recent status into readable content. This was a real interface finding, not merely a typography issue.
- Removed the decorative back-button arrow, constrained prose width, and changed mobile navigation to two columns so the fourth destination stays visible.
- Permission scope, self-reported client identity, and uncertain message outcomes affect user decisions. Keep those explanations even while removing redundant subtitles.
- Account actions belong in the account menu and independent account flow. Client attachment belongs with the assistant whose permissions it inherits.
- Simulation controls belong only in the prototype toolbox. Production must show actual session and bridge observations, and must not imply successful processing from a successful wake.

## Project rules

Root `AGENTS.md` preserves the requested sentence and the explicit bans on redundant labels, decorative all-caps, arbitrary heading emphasis, and decorative numbering. It also requires purposeful grouping, consistent action names, concrete recovery copy, restrained motion, accessible controls, readable line lengths, predictable CSS specificity, and desktop/mobile rendered review.

The approved visual system remains the baseline. The skill's examples of overused palettes are not an instruction to replace a user-approved palette.

## Verification boundary

The native asset/CSP check passed during this review. The UI reviewer inspected Chinese and English desktop/mobile dashboard screenshots served over HTTP with synthetic responses, including permissions, sessions, empty states, and horizontal overflow. Account screenshots and targeted interaction checks are still being completed. Production deployment and actual OAuth or message processing remain separate acceptance checks.
