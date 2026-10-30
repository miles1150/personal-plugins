# Miles Works browser UI

The production UI consumes the reviewed Miles Works foundations from `designs/miles-works-review/system/tokens/foundations.css`, following the baoyu-design workflow pinned in that review. This is the actual service UI, implemented with the existing native HTML and JavaScript stack; it does not load the review prototype or its demo data.

`tokens.css` is the canonical deployed CSS token file. Dashboard styles import it at `/dashboard/tokens.css`; account and Keys styles import the same bytes at `/account/tokens.css`. The retired standalone Waypost marketing website has been removed; it is not part of the unified Worker asset build. The system remains light-only, with 44px controls, visible 3px focus, reduced-motion support and local system fonts.

The account page lists existing assistant and connection identities using the actual paginated account APIs. It selects an existing primary identity when available; it never creates an identity or grants access just by loading. Technical ID inputs and legacy connection, OAuth device, claim/unlink and recovery controls remain available. Creating a named assistant is explicit and keeps the returned stable IDs available even if reloading the list fails. A primary assistant does not alter connection recipients or permissions.

`key-permissions.js` implements the shared permission review for standalone Keys and combined consent. No permissions start selected. The trusted preset visibly selects read-all, write-all and create, bounded by the requested OAuth scope. All-entry read/write includes existing and future entries and requires an additional explicit acknowledgement before final approval. The trusted view shows one concise read/change/create summary; technical client details and individual controls stay in disclosures. Choosing specific entries opens the granular controls automatically. Entry-only access remains available. The UI never requests secret values to render permissions; it uses vault metadata only. Unified consent can approve messages with no keys selection. Keys revocation preserves message access; revoking an entire OAuth connection remains in the existing connection controls.

Validation from the Waypost directory:

```sh
python3 dashboard/check.py
node --test tests/account_ui.mjs tests/keys_ui.mjs tests/dashboard_ui.mjs
```

The current account and dashboard sources support English/Chinese controls. Historical native Chromium checks used synthetic API responses; they do not prove a deployed backend or a real client round trip. The new assistant/session design remains a separate prototype until implemented.
