-- A receiver proves its own credential identity before a bridge dispatches.
-- No bearer tokens or message bodies are stored in this table.
CREATE TABLE IF NOT EXISTS receiver_bindings (
 binding_id TEXT PRIMARY KEY,
 space_id TEXT NOT NULL, actor_id TEXT NOT NULL, consumer_id TEXT NOT NULL,
 request_id TEXT NOT NULL, target TEXT NOT NULL,
 created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, confirmed_at INTEGER,
 receiver_token_id TEXT,
 UNIQUE(space_id,actor_id,consumer_id,request_id),
 FOREIGN KEY(space_id,actor_id) REFERENCES actors(space_id,actor_id),
 FOREIGN KEY(space_id,consumer_id) REFERENCES consumers(space_id,consumer_id)
);
CREATE INDEX IF NOT EXISTS receiver_bindings_expiry ON receiver_bindings(expires_at);
