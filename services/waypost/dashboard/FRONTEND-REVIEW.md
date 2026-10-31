# Production UI review

Reviewed the actual native dashboard and account pages against the repository principles and Anthropic's [frontend-design skill](https://github.com/anthropics/skills/blob/main/skills/frontend-design/SKILL.md). The approved Miles Works colors and system fonts remain in use.

The assistant detail now separates the client source, receiving capability and recent report instead of presenting an unexplained string of technical states. A revoked attachment is shown as unable to receive work. Mobile navigation uses two visible rows; the back action has no decorative arrow. The account consent page retains the client identity warning and the scope of bootstrap access, with technical details collapsed. No decorative eyebrow, numbered cards or partial headline emphasis was found in these pages.

Synthetic HTTP Chromium checks rendered Chinese and English desktop and mobile assistant detail, permissions, session and bridge empty states, the account consent screen, and pending mount plus permission-error states. Screenshots were inspected for desktop and mobile; all four language/size combinations were checked for horizontal overflow. The JS DOM tests cover permission conflicts, signed-out state, bootstrap consent, mount request review, revoked sessions and bridge poll wording. These checks use synthetic responses and do not establish a deployed service or real Google/client round trip.

Bridge status is based on the last successful poll, not a live online claim. Removing a default session offers the server-suggested replacement, another eligible session or no default in one request. Independent owner vault web management and message attachment controls are not implemented in this UI.
