//! Assistant-scoped key authorization. The control database is authoritative;
//! the key-store snapshot prevents older concurrent requests rolling it back.
use super::keys::tools::{KeysPrincipal, KeysTools};
use super::works_v2::WorksPrincipal;
use super::*;
use crate::gateway::mcp::ToolCatalog;

#[derive(Clone)]
pub(crate) struct WorksKeysContext {
    pub(super) request: Request,
    pub(super) principal: WorksPrincipal,
}

impl<D: Database, O: ObjectStore> App<D, O> {
    pub(super) async fn works_keys_recheck(&self, w: &WorksKeysContext) -> Result<()> {
        let now = self
            .works_v2_authorize(&w.request, &w.principal.assistant_id)
            .await?;
        let p = &w.principal;
        if now.account_id != p.account_id
            || now.installation_id != p.installation_id
            || now.family_id != p.family_id
            || now.policy_revision != p.policy_revision
            || now.keys_read != p.keys_read
            || now.keys_write != p.keys_write
            || now.keys_create != p.keys_create
            || now.key_scope != p.key_scope
        {
            return Err(Error::unauthenticated());
        }
        Ok(())
    }

    pub(super) async fn works_keys_call(
        &self,
        request: &Request,
        assistant_id: &str,
        name: &str,
        args: Value,
    ) -> Result<Value> {
        self.keys_config()?;
        let p = self.works_v2_authorize(request, assistant_id).await?;
        if p.key_scope != "all" {
            return Err(Error::new(
                "KEY_SCOPE_UNSUPPORTED",
                "This key scope is not supported yet.",
            ));
        }
        match name {
            "list_entries" if p.keys_read || p.keys_write => (),
            "fetch_entry" if p.keys_read => (),
            "update_entry" if p.keys_write => (),
            "create_entry" if p.keys_create => (),
            "list_entries" | "fetch_entry" | "update_entry" | "create_entry" => {
                return Err(Error::forbidden());
            }
            _ => return Err(Error::not_found()),
        }
        let rows = self
            .db
            .query(Statement::new(
                "SELECT issuer,subject FROM accounts WHERE account_id=? AND disabled=0",
                vec![p.account_id.clone().into()],
            ))
            .await?;
        let identity = rows.first().ok_or_else(Error::unauthenticated)?;
        let issuer = text(identity, "issuer")?;
        let subject = text(identity, "subject")?;
        let rows = self.keys_store().batch(vec![
            Statement::new("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES(?,?,?,?) ON CONFLICT(issuer,subject) DO NOTHING", vec![random_id("account")?.into(), issuer.clone().into(), subject.clone().into(), self.clock.now_ms().into()]),
            Statement::new("SELECT account_id FROM accounts WHERE issuer=? AND subject=? AND disabled=0", vec![issuer.into(), subject.into()]),
        ]).await?;
        let account = text(
            rows[1].first().ok_or_else(Error::unauthenticated)?,
            "account_id",
        )?;
        let rows = self.keys_store().batch(vec![Statement::new("INSERT INTO works_key_policy_cache(assistant_id,account_id,revision,keys_read,keys_write,keys_create,key_scope) VALUES(?,?,?,?,?,?,?) ON CONFLICT(assistant_id) DO UPDATE SET account_id=excluded.account_id,revision=excluded.revision,keys_read=excluded.keys_read,keys_write=excluded.keys_write,keys_create=excluded.keys_create,key_scope=excluded.key_scope WHERE excluded.revision>works_key_policy_cache.revision AND excluded.account_id=works_key_policy_cache.account_id RETURNING revision", vec![p.assistant_id.clone().into(),account.clone().into(),p.policy_revision.into(),i64::from(p.keys_read).into(),i64::from(p.keys_write).into(),i64::from(p.keys_create).into(),p.key_scope.clone().into()])]).await?;
        let _ = rows;
        let context = WorksKeysContext {
            request: request.clone(),
            principal: p.clone(),
        };
        let principal = KeysPrincipal {
            unified: None,
            works: Some(context),
            token_digest: vec![],
            family_id: p.family_id.clone(),
            account_id: account,
            client_id: p.installation_id.clone(),
            request_id: p.assistant_id.clone(),
            read: p.keys_read,
            write: p.keys_write,
            can_create: p.keys_create,
            all_read: p.keys_read,
            all_write: p.keys_write,
        };
        KeysTools(self).call(&principal, name, args).await
    }
}
