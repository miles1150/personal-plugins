# Account/connection UX correction

Uses the existing Miles Works neutral/terracotta tokens and baoyu-design's code-first, task-focused review guidance. Screenshots come from actual account HTML/CSS/JS rendered in isolated Chromium over HTTP, with synthetic API responses. No production cookie or OAuth request was used.

Source: `services/waypost/dashboard/account.{html,css,js}`, `account-i18n.js`, shared `key-permissions.js`. Review: `index.html`.

Verified 12 browser combinations: zh/en × 390/1280px × signed-out/existing assistant/first-time. No page errors or horizontal overflow. Eight focused workflow checks cover persistence, explicit first setup, existing identity reuse, ambiguous choices, error recovery, and language switching without network calls or grant resets. Existing account/keys checks remain intact. Actual Google login and host connection are not inferred from these results.

Current user-session completion is unknown. Deployment must preserve existing secrets, bindings, assets, domain and query-string redaction; no migration or credential rotation required.
