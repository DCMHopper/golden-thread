pub const MIGRATIONS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS imports (
      id TEXT PRIMARY KEY,
      imported_at INTEGER NOT NULL,
      source_filename TEXT NOT NULL,
      source_hash TEXT NOT NULL,
      detected_version TEXT,
      status TEXT NOT NULL,
      stats_json TEXT
    );

    CREATE TABLE IF NOT EXISTS threads (
      id TEXT PRIMARY KEY,
      name TEXT,
      last_message_at INTEGER,
      avatar_attachment_hash TEXT
    );

    CREATE TABLE IF NOT EXISTS recipients (
      id TEXT PRIMARY KEY,
      phone_e164 TEXT,
      profile_name TEXT,
      contact_name TEXT
    );

    CREATE TABLE IF NOT EXISTS thread_members (
      thread_id TEXT NOT NULL,
      recipient_id TEXT NOT NULL,
      PRIMARY KEY (thread_id, recipient_id)
    );

    CREATE TABLE IF NOT EXISTS messages (
      id TEXT PRIMARY KEY,
      thread_id TEXT NOT NULL,
      sender_id TEXT,
      sent_at INTEGER,
      received_at INTEGER,
      type TEXT NOT NULL,
      body TEXT,
      is_outgoing INTEGER NOT NULL DEFAULT 0,
      is_view_once INTEGER NOT NULL DEFAULT 0,
      quote_message_id TEXT,
      metadata_json TEXT,
      dedupe_key TEXT UNIQUE
    );

    CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_dedupe_key ON messages(dedupe_key);

    CREATE TABLE IF NOT EXISTS attachments (
      id TEXT PRIMARY KEY,
      message_id TEXT NOT NULL,
      sha256 TEXT NOT NULL,
      mime TEXT,
      size_bytes INTEGER,
      original_filename TEXT,
      kind TEXT,
      width INTEGER,
      height INTEGER,
      duration_ms INTEGER
    );

    CREATE INDEX IF NOT EXISTS idx_attachments_message_id ON attachments(message_id);
    CREATE INDEX IF NOT EXISTS idx_attachments_sha256 ON attachments(sha256);
    CREATE UNIQUE INDEX IF NOT EXISTS idx_attachments_message_sha ON attachments(message_id, sha256);

    CREATE TABLE IF NOT EXISTS reactions (
      message_id TEXT NOT NULL,
      reactor_id TEXT NOT NULL,
      emoji TEXT NOT NULL,
      reacted_at INTEGER,
      PRIMARY KEY (message_id, reactor_id, emoji)
    );

    CREATE INDEX IF NOT EXISTS idx_messages_thread_id ON messages(thread_id, sent_at DESC);
    CREATE INDEX IF NOT EXISTS idx_messages_sender_id ON messages(sender_id);

    CREATE VIRTUAL TABLE IF NOT EXISTS message_fts USING fts5(
      message_id UNINDEXED,
      thread_id UNINDEXED,
      sender_id UNINDEXED,
      body
    );
    "#,
    r#"
    ALTER TABLE attachments ADD COLUMN size_bucket INTEGER;

    UPDATE attachments
    SET size_bucket = CASE
      WHEN size_bytes IS NULL THEN NULL
      WHEN size_bytes < 1048576 THEN 0
      WHEN size_bytes < 10485760 THEN 1
      ELSE 2
    END;

    CREATE INDEX IF NOT EXISTS idx_attachments_size_bucket_message
      ON attachments(size_bucket, message_id);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS tags (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL UNIQUE,
      color TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      display_order INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS message_tags (
      message_id TEXT NOT NULL,
      tag_id TEXT NOT NULL,
      tagged_at INTEGER NOT NULL,
      PRIMARY KEY (message_id, tag_id),
      FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE,
      FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_message_tags_tag_id
      ON message_tags(tag_id, tagged_at DESC);
    CREATE INDEX IF NOT EXISTS idx_message_tags_message_id
      ON message_tags(message_id);
    CREATE INDEX IF NOT EXISTS idx_tags_display_order
      ON tags(display_order ASC);
    "#,
    r#"
    CREATE INDEX IF NOT EXISTS idx_message_tags_message_id_tagged_at
      ON message_tags(message_id, tagged_at DESC);
    "#,
    r#"
    CREATE UNIQUE INDEX IF NOT EXISTS idx_imports_source_hash
      ON imports(source_hash);
    "#,
    r#"
    ALTER TABLE messages ADD COLUMN sort_ts INTEGER NOT NULL DEFAULT 0;

    UPDATE messages
    SET sort_ts = COALESCE(sent_at, received_at, 0);

    CREATE INDEX IF NOT EXISTS idx_messages_thread_sort
      ON messages(thread_id, sort_ts DESC, id DESC);
    "#,
    r#"
    CREATE TRIGGER IF NOT EXISTS trg_messages_sort_ts
    AFTER INSERT ON messages
    FOR EACH ROW
    WHEN NEW.sort_ts IS NULL OR NEW.sort_ts = 0
    BEGIN
      UPDATE messages
      SET sort_ts = COALESCE(NEW.sent_at, NEW.received_at, 0)
      WHERE id = NEW.id;
    END;
    "#,
];
