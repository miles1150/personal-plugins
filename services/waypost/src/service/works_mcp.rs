//! V2 MCP surface: stable catalog with authorization resolved per call.
use super::*;
use crate::gateway::mcp::{ToolSpec, schema};
use crate::json::fields;

pub(super) fn tools() -> Vec<ToolSpec> {
    let string = || json!({"type":"string"});
    let number = || json!({"type":"integer"});
    let boolean = || json!({"type":"boolean"});
    let mut out = Vec::new();
    let mut add = |name, description, mut props: Value, required: &[&str], read_only| {
        if !matches!(
            name,
            "works_assistant_request" | "works_mount_request" | "works_mount_status"
        ) {
            props
                .as_object_mut()
                .unwrap()
                .insert("assistant_id".into(), string());
        }
        out.push(ToolSpec {
            name,
            description,
            input_schema: schema(props, required),
            read_only,
        });
    };
    add(
        "works_assistant_request",
        "Create an assistant draft without permissions.",
        json!({"name":string()}),
        &["name"],
        false,
    );
    add(
        "works_mount_request",
        "Request owner approval to mount an assistant.",
        json!({"assistant_id":string()}),
        &["assistant_id"],
        false,
    );
    add(
        "works_mount_status",
        "Check this client's mount request.",
        json!({"request_id":string()}),
        &["request_id"],
        true,
    );
    add(
        "works_sessions_register",
        "Register a work session.",
        json!({"host_kind":string(),"external_session_id":string(),"display_name":string(),"capability":{"type":"string","enum":["pull","wakeable"]},"discoverable":boolean()}),
        &["host_kind", "external_session_id", "display_name"],
        false,
    );
    add(
        "works_sessions_list",
        "List the assistant's work sessions.",
        json!({}),
        &[],
        true,
    );
    add(
        "works_sessions_update",
        "Update a session or the assistant's default session.",
        json!({"session_id":string(),"default_session_id":{"type":["string","null"]},"display_name":string(),"discoverable":boolean(),"status":{"type":"string","enum":["registered","paused","deleted"]},"set_default":boolean()}),
        &[],
        false,
    );
    add(
        "works_recipients_list",
        "List available recipients and discoverable sessions.",
        json!({}),
        &[],
        true,
    );
    add(
        "works_messages_send",
        "Send a message to an assistant and optionally a session.",
        json!({"recipient_assistant_id":string(),"session_id":string(),"sender_session_id":string(),"conversation_id":string(),"idempotency_key":string(),"body":string(),"attachments":{"type":"array","items":{"type":"string"}}}),
        &[
            "recipient_assistant_id",
            "conversation_id",
            "idempotency_key",
            "body",
        ],
        false,
    );
    add(
        "works_attachments_prepare",
        "Reserve an encrypted attachment before sending a message.",
        json!({"request_id":string(),"content_type":string(),"size_bytes":number(),"sha256":string(),"filename":string()}),
        &["request_id", "content_type", "size_bytes", "sha256"],
        false,
    );
    add(
        "works_attachments_upload",
        "Upload a reserved attachment as unpadded base64url.",
        json!({"attachment_id":string(),"content_base64":string()}),
        &["attachment_id", "content_base64"],
        false,
    );
    add(
        "works_attachments_read",
        "Read an attachment linked to an accessible message; content is unpadded base64url.",
        json!({"message_id":string(),"attachment_id":string()}),
        &["message_id", "attachment_id"],
        true,
    );
    add(
        "works_materials_list",
        "List shared material collections.",
        json!({}),
        &[],
        true,
    );
    add(
        "works_materials_read",
        "Read a shared material snapshot.",
        json!({"collection_id":string(),"revision":number()}),
        &["collection_id"],
        true,
    );
    add(
        "works_materials_publish",
        "Publish a complete shared material snapshot using expected revision; content uses standard padded base64.",
        json!({"collection_id":string(),"request_id":string(),"expected_revision":{"type":"integer","minimum":0},"items":{"type":"array","maxItems":64,"items":{"type":"object","properties":{"item_id":string(),"path":string(),"content_base64":string(),"metadata":{"type":"object"}},"required":["item_id","path","content_base64"],"additionalProperties":false}}}),
        &["collection_id", "request_id", "expected_revision", "items"],
        false,
    );
    add(
        "works_messages_read",
        "Read assigned deliveries for a session.",
        json!({"session_id":string()}),
        &["session_id"],
        true,
    );
    add(
        "works_delegation_check",
        "Check current owner-approved A2A delegation for an assigned delivery; no approval is created.",
        json!({"session_id":string(),"delivery_id":string(),"assignment_generation":number()}),
        &["session_id", "delivery_id", "assignment_generation"],
        true,
    );
    add(
        "works_deliveries_claim",
        "Claim an assigned delivery.",
        json!({"session_id":string(),"delivery_id":string(),"assignment_generation":number(),"attempt_id":string(),"lease_seconds":number()}),
        &[
            "session_id",
            "delivery_id",
            "assignment_generation",
            "attempt_id",
        ],
        false,
    );
    add(
        "works_deliveries_renew",
        "Renew a delivery claim.",
        json!({"session_id":string(),"delivery_id":string(),"assignment_generation":number(),"claim_generation":number(),"conversation_generation":number(),"lease_seconds":number()}),
        &[
            "session_id",
            "delivery_id",
            "assignment_generation",
            "claim_generation",
            "conversation_generation",
        ],
        false,
    );
    add(
        "works_deliveries_complete",
        "Complete a delivery claim.",
        json!({"session_id":string(),"delivery_id":string(),"assignment_generation":number(),"claim_generation":number(),"conversation_generation":number(),"outcome":string(),"lease_seconds":number()}),
        &[
            "session_id",
            "delivery_id",
            "assignment_generation",
            "claim_generation",
            "conversation_generation",
            "outcome",
        ],
        false,
    );
    add(
        "works_wake_create",
        "Request a wake binding for a session.",
        json!({"session_id":string(),"request_id":string(),"target":string()}),
        &["session_id", "request_id", "target"],
        false,
    );
    add(
        "works_wake_confirm",
        "Confirm a wake binding from the execution installation.",
        json!({"binding_id":string(),"challenge":string()}),
        &["binding_id", "challenge"],
        false,
    );
    add(
        "works_wake_status",
        "Inspect a wake binding.",
        json!({"binding_id":string()}),
        &["binding_id"],
        true,
    );
    add(
        "works_wake_poll",
        "Poll wake references.",
        json!({"binding_id":string(),"cursor":string()}),
        &["binding_id"],
        false,
    );
    add(
        "works_wake_check",
        "Check an assigned wake reference.",
        json!({"binding_id":string(),"delivery_id":string(),"assignment_generation":number()}),
        &["binding_id", "delivery_id", "assignment_generation"],
        true,
    );
    out
}

impl<D: Database, O: ObjectStore> App<D, O> {
    pub(super) async fn works_mcp_call(
        &self,
        request: &Request,
        name: &str,
        mut args: Value,
    ) -> Result<Value> {
        let operation = match name {
            "works_identity" => Some("identity"),
            "works_assistant_request" => Some("assistant_request"),
            "works_mount_request" => Some("mount_request"),
            "works_mount_status" => Some("mount_status"),
            _ => None,
        };
        if let Some(operation) = operation {
            let allowed: &[&str] = match operation {
                "identity" => &[],
                "assistant_request" => &["name"],
                "mount_request" => &["assistant_id"],
                _ => &["request_id"],
            };
            fields(&args, allowed)?;
            return self
                .works_v2_bootstrap_action(request, operation, args)
                .await;
        }
        let key = matches!(
            name,
            "list_entries" | "fetch_entry" | "create_entry" | "update_entry"
        );
        let spec = if key {
            use crate::gateway::mcp::ToolCatalog;
            super::keys::tools::KeysTools(self)
                .tools()
                .into_iter()
                .find(|s| s.name == name)
        } else {
            tools().into_iter().find(|s| s.name == name)
        }
        .ok_or_else(Error::not_found)?;
        let mut allowed: Vec<&str> = spec.input_schema["properties"]
            .as_object()
            .ok_or_else(Error::storage)?
            .keys()
            .map(String::as_str)
            .collect();
        if key {
            allowed.push("assistant_id");
        }
        fields(&args, &allowed)?;
        let object = args
            .as_object_mut()
            .ok_or_else(|| Error::invalid("Expected an object."))?;
        let explicit = match object.remove("assistant_id") {
            Some(Value::String(s)) if !s.is_empty() => Some(s),
            Some(_) => return Err(Error::invalid("assistant_id must be a non-empty string.")),
            None => None,
        };
        let assistant = if let Some(id) = explicit {
            id
        } else {
            let identity = self
                .works_v2_bootstrap_action(request, "identity", json!({}))
                .await?;
            let assistants = identity["assistants"]
                .as_array()
                .ok_or_else(Error::storage)?;
            if assistants.len() != 1 {
                return Err(Error::new(
                    "ASSISTANT_REQUIRED",
                    "Choose an assistant when there is not exactly one active attachment.",
                ));
            }
            assistants[0]["assistant_id"]
                .as_str()
                .ok_or_else(Error::storage)?
                .to_owned()
        };
        if key {
            return self.works_keys_call(request, &assistant, name, args).await;
        }
        let principal = self.works_v2_authorize(request, &assistant).await?;
        let operation = name.strip_prefix("works_").ok_or_else(Error::not_found)?;
        if operation.starts_with("wake_") {
            self.works_wake_dispatch(&principal, operation, args).await
        } else if operation.starts_with("attachments_") {
            self.works_attachments_dispatch(&principal, operation, args)
                .await
        } else if operation.starts_with("materials_") {
            self.works_materials_dispatch(&principal, operation, args)
                .await
        } else {
            self.works_sessions_dispatch(&principal, operation, args)
                .await
        }
    }
}
