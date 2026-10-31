-- Optional verified source session; legacy messages retain NULL.
ALTER TABLE works_messages ADD COLUMN sender_session_id TEXT;
