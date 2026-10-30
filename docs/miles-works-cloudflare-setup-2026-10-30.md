# Cloudflare provisioning receipt — 2026-10-30

## Subsequent application upload, 03:25 UTC

The Mac task reported all five secrets uploaded after backup. Cloud API checks confirmed their names and retained bindings, without reading values. The real compiled application replaced the bootstrap at version `cd04494f-2f48-4633-9f99-836c045e638d`, deployment `f774a351-ba84-43d1-b2ef-155a4fea2b09`, serving 100%. Source implementation is `8e7f15eda70c07a9a600556c3b899c71c1493b67` (the next commit adds this receipt only).

Build hashes (SHA-256):

- `build/index.js`: `3e3644ee12ec120fc7f6d2463b482c1a14e8cd03296c0c950b258d8d13a74494`
- `build/index_bg.wasm`: `7d3de5dfc3b5c952e53392bb1f4dabde5a1d9a63973ff3ff23a7a510ed2aa05c`

Custom domain `works.miles1150.top` is now attached to the new Worker (domain ID `f4d76cb33a7cc5492a825bbe8d81f1815ee108ec`). Legacy domain mappings are unchanged. Static assets are **not uploaded**: asset-session creation succeeded but the asset upload endpoint returned401 through the available cloud tool path. The application upload itself succeeded with inherited secrets. Use authenticated Wrangler on the Mac to deploy the normal application-plus-assets bundle and preserve remote variables (`--keep-vars`). Do not reset credentials to resolve the asset-tool limitation.

This execution environment cannot make public requests to the new hostname: its network policy returns `requests to works.miles1150.top are not allowed`. That is not an HTTP result from the Worker. Public health/metadata, live Google callback registration/login and device acceptance must be checked from the Mac. No public usability success is claimed yet.

For the Mac's actual configuration, fill the three resource IDs below into the existing production example. Preserve the already-configured owner restriction. If omitting the entire example `[vars]` section in order to retain all existing remote variables, use `wrangler deploy --keep-vars`; do not leave the owner placeholder in a configuration that will be uploaded. The five secrets already exist and do not need to be re-uploaded.

The following sections record the earlier provisioning state, before secret and application upload.

User authorized local generation, backup and upload of the five deployment secrets. The coordinating task reports the Mac backup was persisted and reread successfully; this cloud task has not inspected any secret value.

## Created and checked

Account: `cd563920285e2c8a3fbe0775fbca87d3`.

| Binding | New resource | ID |
|---|---|---|
| Worker | `miles-works-production` | — |
| `DB` | `miles-works-control-production` | `34005d49-ce5e-4dfd-a391-5032370ff841` |
| `KEYS_DB` | `miles-works-keys-production` | `7237d31d-e230-4fb0-81a2-41cd06b1ccff` |
| `OBJECTS` | `miles-works-production-objects` | — |

Both D1 databases received all 20 migrations; a subsequent read confirmed count20 and final `0020_receiver_claims.sql`. Resource IDs differ from each other and from both legacy databases. Creation requested APAC.

The Worker currently runs an explicit 503 bootstrap response, not the application. `workers.dev` and preview URLs are disabled; no custom domain has been attached. Production mode, unified product, disabled operator mode, account onboarding, DCR, signup limit1, verified owner restriction and rate limits3001/3002 are configured. Public origin is prepared as `https://works.miles1150.top`. No secret values are present in this receipt or repository.

At the final provisioning check the new Worker secret-name list was empty. The Mac task can now upload the five backed-up secrets to this exact Worker. Preserve the configured variables and bindings when uploading application code. Use the production example with these IDs, and retain the verified owner restriction already present in Cloudflare; do not replace it with the example placeholder.

## Still required

- Mac upload receipt for all five secret names, without values.
- Verified Google client callback registration for `https://works.miles1150.top/account/callback`. Local OIDC JSON alone does not prove registration.
- Actual application Wasm/assets deployment, custom-domain activation and live acceptance.
- Host/device verification before changing installed plugin URLs or retiring anything.

Legacy Workers, domains, data and credentials remain unchanged. The earlier implementation checkpoint describes the state before this provisioning step.
