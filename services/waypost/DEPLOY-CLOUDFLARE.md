# Deploy Miles Works to Cloudflare

The supported hosted service is **Miles Works**, at <https://works.miles1150.top>;
its sole public MCP resource is `https://works.miles1150.top/mcp`.

Use [the unified deployment guide](docs/unified-deployment.md) and
[wrangler.unified.production.example.toml](wrangler.unified.production.example.toml).
The standalone Waypost and Keys production templates and hosted services have
been retired. Do not recreate them or send their old credentials to the new origin.
The local `wrangler.toml` remains a CI compatibility fixture, not a production profile.

For the existing deployment, reuse its reviewed Worker, DB, KEYS_DB and OBJECTS
bindings and secrets. Do not run fresh-install resource or key generation steps on
an existing installation. Check candidate configuration, assets and schema compatibility
before activation; preserve query-string redaction and existing secret values.

The unified guide distinguishes fresh installation, existing-service updates,
acceptance and rollback. `/health` alone does not prove OAuth, message delivery or
vault access. Never place secret values in command arguments, logs or source control.
