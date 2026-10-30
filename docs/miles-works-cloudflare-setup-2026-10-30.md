# Cloudflare provisioning receipt — 2026-10-30

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
