-- Claims coordinate cooperating receivers; receipts remain delivery/processing evidence.
CREATE TABLE receiver_claims (
 space_id TEXT NOT NULL,actor_id TEXT NOT NULL,consumer_id TEXT NOT NULL,item_id TEXT NOT NULL,
 attempt_id TEXT NOT NULL,generation INTEGER NOT NULL CHECK(generation>0),
 request_id TEXT NOT NULL,lease_expires_at INTEGER NOT NULL,completed_at INTEGER,
 PRIMARY KEY(space_id,actor_id,consumer_id,item_id),
 FOREIGN KEY(space_id,actor_id) REFERENCES actors(space_id,actor_id),
 FOREIGN KEY(space_id,consumer_id) REFERENCES consumers(space_id,consumer_id),
 FOREIGN KEY(space_id,item_id) REFERENCES feed_items(space_id,item_id)
);
-- Keep denied and superseded attempts so a retry cannot silently become a new claim.
CREATE TABLE receiver_claim_attempts (
 space_id TEXT NOT NULL,actor_id TEXT NOT NULL,consumer_id TEXT NOT NULL,item_id TEXT NOT NULL,
 attempt_id TEXT NOT NULL,lease_seconds INTEGER NOT NULL CHECK(lease_seconds BETWEEN 1 AND 300),
 generation INTEGER NOT NULL,lease_expires_at INTEGER NOT NULL,granted INTEGER NOT NULL CHECK(granted IN(0,1)),
 PRIMARY KEY(space_id,actor_id,consumer_id,item_id,attempt_id),
 FOREIGN KEY(space_id,actor_id,consumer_id,item_id) REFERENCES receiver_claims(space_id,actor_id,consumer_id,item_id)
);
