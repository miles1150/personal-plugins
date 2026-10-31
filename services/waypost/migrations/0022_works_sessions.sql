-- Account-scoped v2 routing. Old service tables are deliberately untouched.
CREATE TABLE works_sessions (
 session_id TEXT PRIMARY KEY, account_id TEXT NOT NULL, assistant_id TEXT NOT NULL,
 installation_id TEXT NOT NULL, host_kind TEXT NOT NULL, external_session_id TEXT NOT NULL,
 display_name TEXT NOT NULL, capability TEXT NOT NULL CHECK(capability IN ('pull','wakeable')),
 discoverable INTEGER NOT NULL DEFAULT 1, status TEXT NOT NULL DEFAULT 'registered' CHECK(status IN ('registered','paused','deleted')),
 created_at INTEGER NOT NULL, last_seen_at INTEGER NOT NULL, route_revision INTEGER NOT NULL DEFAULT 1, mutation_token TEXT
);
CREATE UNIQUE INDEX works_sessions_live_host ON works_sessions(installation_id,host_kind,external_session_id) WHERE status <> 'deleted';
CREATE INDEX works_sessions_owner ON works_sessions(account_id,assistant_id,status);
CREATE TABLE works_conversations (
 account_id TEXT NOT NULL, conversation_id TEXT NOT NULL, assistant_id TEXT NOT NULL,
 owner_session_id TEXT, active_delivery_id TEXT, generation INTEGER NOT NULL DEFAULT 0,
 PRIMARY KEY(account_id,conversation_id,assistant_id)
);
CREATE TABLE works_messages (
 message_id TEXT PRIMARY KEY, account_id TEXT NOT NULL, sender_assistant_id TEXT NOT NULL,
 conversation_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, fingerprint TEXT NOT NULL,
 ciphertext TEXT NOT NULL, created_at INTEGER NOT NULL,
 UNIQUE(account_id,sender_assistant_id,idempotency_key)
);
CREATE TABLE works_deliveries (
 delivery_id TEXT PRIMARY KEY, account_id TEXT NOT NULL, message_id TEXT NOT NULL UNIQUE,
 assistant_id TEXT NOT NULL, conversation_id TEXT NOT NULL, target_mode TEXT NOT NULL CHECK(target_mode IN ('assistant','session')),
 requested_session_id TEXT, assigned_session_id TEXT, assignment_generation INTEGER NOT NULL DEFAULT 1,
 state TEXT NOT NULL CHECK(state IN ('waiting_for_session','assigned','claimed','completed','failed','needs_handoff','unknown')),
 claim_attempt_id TEXT, claim_generation INTEGER NOT NULL DEFAULT 0, conversation_generation INTEGER NOT NULL DEFAULT 0,
 lease_expires_at INTEGER, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE INDEX works_delivery_inbox ON works_deliveries(account_id,assistant_id,assigned_session_id,state,created_at);
-- Establish continuity as soon as a previously unassigned backlog is routed.
CREATE TRIGGER works_delivery_backfill_owner AFTER UPDATE OF state ON works_deliveries
WHEN OLD.state='waiting_for_session' AND NEW.state='assigned' AND NEW.assigned_session_id IS NOT NULL
BEGIN
 INSERT INTO works_conversations(account_id,conversation_id,assistant_id,owner_session_id)
 VALUES(NEW.account_id,NEW.conversation_id,NEW.assistant_id,NEW.assigned_session_id)
 ON CONFLICT(account_id,conversation_id,assistant_id) DO UPDATE SET owner_session_id=COALESCE(works_conversations.owner_session_id,excluded.owner_session_id);
END;
CREATE TABLE works_handoff_audit (
 event_id TEXT PRIMARY KEY, account_id TEXT NOT NULL, assistant_id TEXT NOT NULL,
 delivery_id TEXT NOT NULL, destination_session_id TEXT NOT NULL,
 assignment_generation INTEGER NOT NULL, conversation_generation INTEGER NOT NULL,
 created_at INTEGER NOT NULL
);
