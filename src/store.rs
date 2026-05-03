use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::Serialize;

use crate::jid::chat_kind;

#[derive(Debug)]
pub struct Store {
    conn: Connection,
    fts_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRecord {
    pub rowid: i64,
    pub chat_jid: String,
    pub chat_name: Option<String>,
    pub msg_id: String,
    pub sender_jid: Option<String>,
    pub sender_name: Option<String>,
    pub ts: i64,
    pub from_me: bool,
    pub text: Option<String>,
    pub display_text: Option<String>,
    pub media_type: Option<String>,
    pub media_caption: Option<String>,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub direct_path: Option<String>,
    pub file_length: Option<i64>,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MediaRecord {
    pub chat_jid: String,
    pub msg_id: String,
    pub media_type: String,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub direct_path: String,
    pub media_key: Vec<u8>,
    pub file_sha256: Vec<u8>,
    pub file_enc_sha256: Vec<u8>,
    pub file_length: i64,
}

#[derive(Debug, Clone)]
pub struct MessageInsert {
    pub chat_jid: String,
    pub chat_name: Option<String>,
    pub msg_id: String,
    pub sender_jid: Option<String>,
    pub sender_name: Option<String>,
    pub ts: i64,
    pub from_me: bool,
    pub text: Option<String>,
    pub display_text: Option<String>,
    pub media_type: Option<String>,
    pub media_caption: Option<String>,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub direct_path: Option<String>,
    pub media_key: Option<Vec<u8>>,
    pub file_sha256: Option<Vec<u8>>,
    pub file_enc_sha256: Option<Vec<u8>>,
    pub file_length: Option<i64>,
    pub raw_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContactUpsert {
    pub jid: String,
    pub lid: Option<String>,
    pub phone_number: Option<String>,
    pub name: Option<String>,
    pub full_name: Option<String>,
    pub first_name: Option<String>,
    pub username: Option<String>,
    pub push_name: Option<String>,
    pub source: String,
    pub from_full_sync: bool,
    pub updated_at: Option<i64>,
    pub last_seen_ts: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct MessageFilter {
    pub chat: Option<String>,
    pub sender: Option<String>,
    pub from_me: Option<bool>,
    pub media_type: Option<String>,
    pub limit: usize,
    pub asc: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRecord {
    pub jid: String,
    pub kind: String,
    pub name: Option<String>,
    pub last_message_ts: Option<i64>,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContactRecord {
    pub jid: String,
    pub lid: Option<String>,
    pub phone_number: Option<String>,
    pub name: Option<String>,
    pub full_name: Option<String>,
    pub first_name: Option<String>,
    pub username: Option<String>,
    pub push_name: Option<String>,
    pub source: String,
    pub from_full_sync: bool,
    pub updated_at: Option<i64>,
    pub last_seen_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreStats {
    pub store_path: String,
    pub chats: i64,
    pub messages: i64,
    pub fts: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsReport {
    pub total_messages: i64,
    pub inbound_messages: i64,
    pub outbound_messages: i64,
    pub chats: Vec<ChatAnalytics>,
    pub senders: Vec<SenderAnalytics>,
    pub media_types: Vec<TypeAnalytics>,
    pub days: Vec<DayAnalytics>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatAnalytics {
    pub chat_jid: String,
    pub chat_name: Option<String>,
    pub messages: i64,
    pub last_message_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SenderAnalytics {
    pub sender_jid: Option<String>,
    pub sender_name: Option<String>,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeAnalytics {
    pub media_type: String,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayAnalytics {
    pub day: String,
    pub messages: i64,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self {
            conn,
            fts_available: false,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_readonly(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("open {} read-only", path.display()))?;
        let fts_available = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'messages_fts'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        Ok(Self {
            conn,
            fts_available,
        })
    }

    fn migrate(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS chats (
                jid TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                name TEXT,
                last_message_ts INTEGER
            );

            CREATE TABLE IF NOT EXISTS messages (
                rowid INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_jid TEXT NOT NULL,
                chat_name TEXT,
                msg_id TEXT NOT NULL,
                sender_jid TEXT,
                sender_name TEXT,
                ts INTEGER NOT NULL,
                from_me INTEGER NOT NULL,
                text TEXT,
                display_text TEXT,
                media_type TEXT,
                media_caption TEXT,
                filename TEXT,
                mime_type TEXT,
                direct_path TEXT,
                media_key BLOB,
                file_sha256 BLOB,
                file_enc_sha256 BLOB,
                file_length INTEGER,
                local_path TEXT,
                downloaded_at INTEGER,
                raw_json TEXT,
                UNIQUE(chat_jid, msg_id)
            );

            CREATE INDEX IF NOT EXISTS idx_messages_chat_ts ON messages(chat_jid, ts);
            CREATE INDEX IF NOT EXISTS idx_messages_ts ON messages(ts);
            CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender_jid);

            CREATE TABLE IF NOT EXISTS contacts (
                jid TEXT PRIMARY KEY,
                lid TEXT,
                phone_number TEXT,
                name TEXT,
                full_name TEXT,
                first_name TEXT,
                username TEXT,
                push_name TEXT,
                source TEXT NOT NULL,
                from_full_sync INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER,
                last_seen_ts INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_contacts_name ON contacts(name);
            CREATE INDEX IF NOT EXISTS idx_contacts_phone ON contacts(phone_number);
            CREATE INDEX IF NOT EXISTS idx_contacts_lid ON contacts(lid);

            UPDATE messages
            SET media_type = 'protocol', display_text = '[protocol]'
            WHERE media_type IS NULL
              AND display_text IS NULL
              AND raw_json LIKE '%"protocol_message":%'
              AND raw_json NOT LIKE '%"protocol_message":null%';
            "#,
        )?;

        self.fts_available = self
            .conn
            .execute(
                "CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(searchable)",
                [],
            )
            .is_ok();
        Ok(())
    }

    pub fn stats(&self, path: &Path) -> Result<StoreStats> {
        Ok(StoreStats {
            store_path: path.display().to_string(),
            chats: self.count("chats")?,
            messages: self.count("messages")?,
            fts: self.fts_available,
        })
    }

    fn count(&self, table: &str) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        self.conn
            .query_row(&sql, [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn upsert_message(&mut self, msg: &MessageInsert) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            INSERT INTO chats (jid, kind, name, last_message_ts)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(jid) DO UPDATE SET
                kind = excluded.kind,
                name = COALESCE(excluded.name, chats.name),
                last_message_ts = MAX(COALESCE(chats.last_message_ts, 0), excluded.last_message_ts)
            "#,
            params![
                msg.chat_jid,
                chat_kind(&msg.chat_jid),
                msg.chat_name,
                msg.ts
            ],
        )?;

        let rowid: i64 = tx.query_row(
            r#"
            INSERT INTO messages (
                chat_jid, chat_name, msg_id, sender_jid, sender_name, ts, from_me,
                text, display_text, media_type, media_caption, filename, mime_type,
                direct_path, media_key, file_sha256, file_enc_sha256, file_length, raw_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ON CONFLICT(chat_jid, msg_id) DO UPDATE SET
                chat_name = COALESCE(excluded.chat_name, messages.chat_name),
                sender_jid = COALESCE(excluded.sender_jid, messages.sender_jid),
                sender_name = COALESCE(excluded.sender_name, messages.sender_name),
                ts = excluded.ts,
                from_me = excluded.from_me,
                text = COALESCE(excluded.text, messages.text),
                display_text = COALESCE(excluded.display_text, messages.display_text),
                media_type = COALESCE(excluded.media_type, messages.media_type),
                media_caption = COALESCE(excluded.media_caption, messages.media_caption),
                filename = COALESCE(excluded.filename, messages.filename),
                mime_type = COALESCE(excluded.mime_type, messages.mime_type),
                direct_path = COALESCE(excluded.direct_path, messages.direct_path),
                media_key = COALESCE(excluded.media_key, messages.media_key),
                file_sha256 = COALESCE(excluded.file_sha256, messages.file_sha256),
                file_enc_sha256 = COALESCE(excluded.file_enc_sha256, messages.file_enc_sha256),
                file_length = COALESCE(excluded.file_length, messages.file_length),
                raw_json = COALESCE(excluded.raw_json, messages.raw_json)
            RETURNING rowid
            "#,
            params![
                msg.chat_jid,
                msg.chat_name,
                msg.msg_id,
                msg.sender_jid,
                msg.sender_name,
                msg.ts,
                i64::from(msg.from_me),
                msg.text,
                msg.display_text,
                msg.media_type,
                msg.media_caption,
                msg.filename,
                msg.mime_type,
                msg.direct_path,
                msg.media_key,
                msg.file_sha256,
                msg.file_enc_sha256,
                msg.file_length,
                msg.raw_json,
            ],
            |row| row.get(0),
        )?;

        if self.fts_available {
            let searchable = searchable_text(msg);
            tx.execute("DELETE FROM messages_fts WHERE rowid = ?1", params![rowid])
                .ok();
            tx.execute(
                "INSERT INTO messages_fts(rowid, searchable) VALUES (?1, ?2)",
                params![rowid, searchable],
            )
            .ok();
        }

        tx.commit()?;
        Ok(rowid)
    }

    pub fn upsert_contact(&mut self, contact: &ContactUpsert) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO contacts (
                jid, lid, phone_number, name, full_name, first_name, username,
                push_name, source, from_full_sync, updated_at, last_seen_ts
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(jid) DO UPDATE SET
                lid = COALESCE(excluded.lid, contacts.lid),
                phone_number = COALESCE(excluded.phone_number, contacts.phone_number),
                name = COALESCE(excluded.name, contacts.name),
                full_name = COALESCE(excluded.full_name, contacts.full_name),
                first_name = COALESCE(excluded.first_name, contacts.first_name),
                username = COALESCE(excluded.username, contacts.username),
                push_name = COALESCE(excluded.push_name, contacts.push_name),
                source = excluded.source,
                from_full_sync = MAX(contacts.from_full_sync, excluded.from_full_sync),
                updated_at = MAX(COALESCE(contacts.updated_at, 0), COALESCE(excluded.updated_at, 0)),
                last_seen_ts = MAX(COALESCE(contacts.last_seen_ts, 0), COALESCE(excluded.last_seen_ts, 0))
            "#,
            params![
                contact.jid,
                contact.lid,
                contact.phone_number,
                contact.name,
                contact.full_name,
                contact.first_name,
                contact.username,
                contact.push_name,
                contact.source,
                if contact.from_full_sync { 1 } else { 0 },
                contact.updated_at,
                contact.last_seen_ts,
            ],
        )?;
        Ok(())
    }

    pub fn list_messages(&self, filter: &MessageFilter) -> Result<Vec<MessageRecord>> {
        let mut sql = String::from("SELECT * FROM messages WHERE 1=1");
        let mut values: Vec<String> = Vec::new();
        append_filters(&mut sql, &mut values, filter);
        sql.push_str(if filter.asc {
            " ORDER BY ts ASC, rowid ASC"
        } else {
            " ORDER BY ts DESC, rowid DESC"
        });
        sql.push_str(" LIMIT ?");
        values.push(filter.limit.max(1).to_string());
        self.query_messages(&sql, values)
    }

    pub fn search_messages(
        &self,
        query: &str,
        filter: &MessageFilter,
    ) -> Result<Vec<MessageRecord>> {
        if self.fts_available {
            let mut sql = String::from(
                "SELECT messages.* FROM messages JOIN messages_fts ON messages.rowid = messages_fts.rowid WHERE messages_fts MATCH ?",
            );
            let mut values = vec![escape_fts_query(query)];
            append_filters_with_prefix(&mut sql, &mut values, filter, "messages");
            sql.push_str(" ORDER BY bm25(messages_fts), messages.ts DESC LIMIT ?");
            values.push(filter.limit.max(1).to_string());
            return self.query_messages(&sql, values);
        }

        let mut sql = String::from(
            "SELECT * FROM messages WHERE (text LIKE ? OR display_text LIKE ? OR media_caption LIKE ? OR filename LIKE ?)",
        );
        let needle = format!("%{query}%");
        let mut values = vec![needle.clone(), needle.clone(), needle.clone(), needle];
        append_filters(&mut sql, &mut values, filter);
        sql.push_str(" ORDER BY ts DESC, rowid DESC LIMIT ?");
        values.push(filter.limit.max(1).to_string());
        self.query_messages(&sql, values)
    }

    pub fn show_message(&self, chat: &str, id: &str) -> Result<Option<MessageRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM messages WHERE chat_jid = ?1 AND msg_id = ?2")?;
        stmt.query_row(params![chat, id], map_message)
            .optional()
            .map_err(Into::into)
    }

    pub fn media_message(&self, chat: &str, id: &str) -> Result<Option<MediaRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT chat_jid, msg_id, media_type, filename, mime_type, direct_path,
                   media_key, file_sha256, file_enc_sha256, file_length
            FROM messages
            WHERE chat_jid = ?1 AND msg_id = ?2
              AND media_type IS NOT NULL
              AND direct_path IS NOT NULL
              AND media_key IS NOT NULL
              AND file_sha256 IS NOT NULL
              AND file_enc_sha256 IS NOT NULL
              AND file_length IS NOT NULL
            "#,
        )?;
        stmt.query_row(params![chat, id], |row| {
            Ok(MediaRecord {
                chat_jid: row.get("chat_jid")?,
                msg_id: row.get("msg_id")?,
                media_type: row.get("media_type")?,
                filename: row.get("filename")?,
                mime_type: row.get("mime_type")?,
                direct_path: row.get("direct_path")?,
                media_key: row.get("media_key")?,
                file_sha256: row.get("file_sha256")?,
                file_enc_sha256: row.get("file_enc_sha256")?,
                file_length: row.get("file_length")?,
            })
        })
        .optional()
        .map_err(Into::into)
    }

    pub fn mark_media_downloaded(&self, chat: &str, id: &str, path: &Path) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET local_path = ?1, downloaded_at = ?2 WHERE chat_jid = ?3 AND msg_id = ?4",
            params![
                path.display().to_string(),
                chrono::Utc::now().timestamp(),
                chat,
                id
            ],
        )?;
        Ok(())
    }

    pub fn update_message_text(&self, chat: &str, id: &str, text: &str) -> Result<bool> {
        let rowid = self
            .conn
            .query_row(
                r#"
                UPDATE messages
                SET text = ?3,
                    display_text = ?3,
                    media_type = NULL,
                    media_caption = NULL,
                    filename = NULL,
                    mime_type = NULL,
                    direct_path = NULL,
                    media_key = NULL,
                    file_sha256 = NULL,
                    file_enc_sha256 = NULL,
                    file_length = NULL,
                    local_path = NULL,
                    downloaded_at = NULL,
                    raw_json = NULL
                WHERE chat_jid = ?1 AND msg_id = ?2
                RETURNING rowid
                "#,
                params![chat, id, text],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(rowid) = rowid {
            self.replace_fts(rowid, text)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn mark_message_revoked(&self, chat: &str, id: &str) -> Result<bool> {
        let display = "Message revoked";
        let rowid = self
            .conn
            .query_row(
                r#"
                UPDATE messages
                SET text = NULL,
                    display_text = ?3,
                    media_type = 'protocol',
                    media_caption = ?3,
                    filename = NULL,
                    mime_type = NULL,
                    direct_path = NULL,
                    media_key = NULL,
                    file_sha256 = NULL,
                    file_enc_sha256 = NULL,
                    file_length = NULL,
                    local_path = NULL,
                    downloaded_at = NULL,
                    raw_json = NULL
                WHERE chat_jid = ?1 AND msg_id = ?2
                RETURNING rowid
                "#,
                params![chat, id, display],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(rowid) = rowid {
            self.replace_fts(rowid, display)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn delete_message(&self, chat: &str, id: &str) -> Result<bool> {
        let rowid = self
            .conn
            .query_row(
                "SELECT rowid FROM messages WHERE chat_jid = ?1 AND msg_id = ?2",
                params![chat, id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(rowid) = rowid else {
            return Ok(false);
        };
        self.conn.execute(
            "DELETE FROM messages WHERE chat_jid = ?1 AND msg_id = ?2",
            params![chat, id],
        )?;
        if self.fts_available {
            self.conn
                .execute("DELETE FROM messages_fts WHERE rowid = ?1", params![rowid])
                .ok();
        }
        Ok(true)
    }

    fn replace_fts(&self, rowid: i64, searchable: &str) -> Result<()> {
        if self.fts_available {
            self.conn
                .execute("DELETE FROM messages_fts WHERE rowid = ?1", params![rowid])
                .ok();
            self.conn
                .execute(
                    "INSERT INTO messages_fts(rowid, searchable) VALUES (?1, ?2)",
                    params![rowid, searchable],
                )
                .ok();
        }
        Ok(())
    }

    pub fn list_media(
        &self,
        chat: Option<&str>,
        media_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MessageRecord>> {
        let mut sql = String::from(
            "SELECT * FROM messages WHERE media_type IN ('image', 'video', 'audio', 'document', 'sticker')",
        );
        let mut values: Vec<String> = Vec::new();
        if let Some(chat) = chat {
            sql.push_str(" AND chat_jid = ?");
            values.push(chat.to_string());
        }
        if let Some(media_type) = media_type {
            sql.push_str(" AND media_type = ?");
            values.push(media_type.to_string());
        }
        sql.push_str(" ORDER BY ts DESC, rowid DESC LIMIT ?");
        values.push(limit.max(1).to_string());
        self.query_messages(&sql, values)
    }

    pub fn message_context(
        &self,
        chat: &str,
        id: &str,
        before: usize,
        after: usize,
    ) -> Result<Vec<MessageRecord>> {
        let Some(anchor) = self.show_message(chat, id)? else {
            return Ok(Vec::new());
        };
        let mut before_rows = self.query_messages(
            "SELECT * FROM messages WHERE chat_jid = ? AND (ts < ? OR (ts = ? AND rowid < ?)) ORDER BY ts DESC, rowid DESC LIMIT ?",
            vec![
                chat.to_string(),
                anchor.ts.to_string(),
                anchor.ts.to_string(),
                anchor.rowid.to_string(),
                before.to_string(),
            ],
        )?;
        before_rows.reverse();

        let after_rows = self.query_messages(
            "SELECT * FROM messages WHERE chat_jid = ? AND (ts > ? OR (ts = ? AND rowid > ?)) ORDER BY ts ASC, rowid ASC LIMIT ?",
            vec![
                chat.to_string(),
                anchor.ts.to_string(),
                anchor.ts.to_string(),
                anchor.rowid.to_string(),
                after.to_string(),
            ],
        )?;

        before_rows.push(anchor);
        before_rows.extend(after_rows);
        Ok(before_rows)
    }

    pub fn list_chats(&self, query: Option<&str>, limit: usize) -> Result<Vec<ChatRecord>> {
        self.list_chats_by_kind(query, limit, None)
    }

    pub fn list_contacts(&self, query: Option<&str>, limit: usize) -> Result<Vec<ChatRecord>> {
        self.list_chats_by_kind(query, limit, Some("dm"))
    }

    pub fn list_contact_book(
        &self,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ContactRecord>> {
        let mut sql = String::from(
            r#"
            SELECT jid, lid, phone_number, name, full_name, first_name, username,
                   push_name, source, from_full_sync, updated_at, last_seen_ts
            FROM contacts
            "#,
        );
        let mut values = Vec::new();
        if let Some(query) = query {
            sql.push_str(
                r#"
                WHERE jid LIKE ?
                   OR lid LIKE ?
                   OR phone_number LIKE ?
                   OR name LIKE ?
                   OR full_name LIKE ?
                   OR first_name LIKE ?
                   OR username LIKE ?
                   OR push_name LIKE ?
                "#,
            );
            let value = format!("%{query}%");
            for _ in 0..8 {
                values.push(value.clone());
            }
        }
        sql.push_str(
            r#"
            ORDER BY
                COALESCE(updated_at, last_seen_ts, 0) DESC,
                COALESCE(name, full_name, first_name, push_name, username, jid) COLLATE NOCASE ASC
            LIMIT ?
            "#,
        );
        values.push(limit.max(1).to_string());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values), map_contact)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn list_chats_by_kind(
        &self,
        query: Option<&str>,
        limit: usize,
        kind: Option<&str>,
    ) -> Result<Vec<ChatRecord>> {
        let mut sql = String::from(
            r#"
            SELECT c.jid, c.kind, c.name, c.last_message_ts, COUNT(m.rowid) AS message_count
            FROM chats c
            LEFT JOIN messages m ON m.chat_jid = c.jid
            "#,
        );
        let mut values = Vec::new();
        let mut clauses = Vec::new();
        if let Some(kind) = kind {
            clauses.push("c.kind = ?");
            values.push(kind.to_string());
        }
        if let Some(query) = query {
            clauses.push("(c.jid LIKE ? OR c.name LIKE ?)");
            let value = format!("%{query}%");
            values.push(value.clone());
            values.push(value);
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(
            " GROUP BY c.jid, c.kind, c.name, c.last_message_ts ORDER BY COALESCE(c.last_message_ts, 0) DESC LIMIT ?",
        );
        values.push(limit.max(1).to_string());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
            Ok(ChatRecord {
                jid: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                last_message_ts: row.get(3)?,
                message_count: row.get(4)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn query_messages(&self, sql: &str, values: Vec<String>) -> Result<Vec<MessageRecord>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), map_message)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn analytics(&self, chat: Option<&str>) -> Result<AnalyticsReport> {
        let (where_sql, values) = match chat {
            Some(chat) => (" WHERE chat_jid = ?1", vec![chat.to_string()]),
            None => ("", Vec::new()),
        };
        let total_messages = self.scalar_i64(
            &format!("SELECT COUNT(*) FROM messages{where_sql}"),
            values.clone(),
        )?;
        let inbound_messages = self.scalar_i64(
            &format!(
                "SELECT COUNT(*) FROM messages{where_sql}{}from_me = 0",
                and_prefix(chat)
            ),
            values.clone(),
        )?;
        let outbound_messages = self.scalar_i64(
            &format!(
                "SELECT COUNT(*) FROM messages{where_sql}{}from_me = 1",
                and_prefix(chat)
            ),
            values.clone(),
        )?;

        Ok(AnalyticsReport {
            total_messages,
            inbound_messages,
            outbound_messages,
            chats: self.chat_analytics(chat)?,
            senders: self.sender_analytics(chat)?,
            media_types: self.type_analytics(chat)?,
            days: self.day_analytics(chat)?,
        })
    }

    fn scalar_i64(&self, sql: &str, values: Vec<String>) -> Result<i64> {
        self.conn
            .query_row(sql, params_from_iter(values.iter()), |row| row.get(0))
            .map_err(Into::into)
    }

    fn chat_analytics(&self, chat: Option<&str>) -> Result<Vec<ChatAnalytics>> {
        let mut sql = String::from(
            r#"
            SELECT chat_jid, MAX(chat_name), COUNT(*), MAX(ts)
            FROM messages
            "#,
        );
        let mut values = Vec::new();
        if let Some(chat) = chat {
            sql.push_str(" WHERE chat_jid = ?");
            values.push(chat.to_string());
        }
        sql.push_str(" GROUP BY chat_jid ORDER BY COUNT(*) DESC LIMIT 100");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
            Ok(ChatAnalytics {
                chat_jid: row.get(0)?,
                chat_name: row.get(1)?,
                messages: row.get(2)?,
                last_message_ts: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn sender_analytics(&self, chat: Option<&str>) -> Result<Vec<SenderAnalytics>> {
        let mut sql = String::from(
            r#"
            SELECT sender_jid, MAX(sender_name), COUNT(*)
            FROM messages
            "#,
        );
        let mut values = Vec::new();
        if let Some(chat) = chat {
            sql.push_str(" WHERE chat_jid = ?");
            values.push(chat.to_string());
        }
        sql.push_str(" GROUP BY sender_jid ORDER BY COUNT(*) DESC LIMIT 100");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
            Ok(SenderAnalytics {
                sender_jid: row.get(0)?,
                sender_name: row.get(1)?,
                messages: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn type_analytics(&self, chat: Option<&str>) -> Result<Vec<TypeAnalytics>> {
        let mut sql = String::from(
            r#"
            SELECT COALESCE(media_type, 'text') AS kind, COUNT(*)
            FROM messages
            "#,
        );
        let mut values = Vec::new();
        if let Some(chat) = chat {
            sql.push_str(" WHERE chat_jid = ?");
            values.push(chat.to_string());
        }
        sql.push_str(" GROUP BY kind ORDER BY COUNT(*) DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
            Ok(TypeAnalytics {
                media_type: row.get(0)?,
                messages: row.get(1)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn day_analytics(&self, chat: Option<&str>) -> Result<Vec<DayAnalytics>> {
        let mut sql = String::from(
            r#"
            SELECT strftime('%Y-%m-%d', ts, 'unixepoch') AS day, COUNT(*)
            FROM messages
            "#,
        );
        let mut values = Vec::new();
        if let Some(chat) = chat {
            sql.push_str(" WHERE chat_jid = ?");
            values.push(chat.to_string());
        }
        sql.push_str(" GROUP BY day ORDER BY day DESC LIMIT 365");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
            Ok(DayAnalytics {
                day: row.get(0)?,
                messages: row.get(1)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn and_prefix(has_where: Option<&str>) -> &'static str {
    if has_where.is_some() {
        " AND "
    } else {
        " WHERE "
    }
}

fn append_filters(sql: &mut String, values: &mut Vec<String>, filter: &MessageFilter) {
    append_filters_with_prefix(sql, values, filter, "");
}

fn append_filters_with_prefix(
    sql: &mut String,
    values: &mut Vec<String>,
    filter: &MessageFilter,
    prefix: &str,
) {
    let col = |name: &str| {
        if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}.{name}")
        }
    };
    if let Some(chat) = &filter.chat {
        sql.push_str(" AND ");
        sql.push_str(&col("chat_jid"));
        sql.push_str(" = ?");
        values.push(chat.clone());
    }
    if let Some(sender) = &filter.sender {
        sql.push_str(" AND ");
        sql.push_str(&col("sender_jid"));
        sql.push_str(" = ?");
        values.push(sender.clone());
    }
    if let Some(from_me) = filter.from_me {
        sql.push_str(" AND ");
        sql.push_str(&col("from_me"));
        sql.push_str(" = ?");
        values.push(i64::from(from_me).to_string());
    }
    if let Some(media_type) = &filter.media_type {
        if media_type == "text" {
            sql.push_str(" AND ");
            sql.push_str(&col("media_type"));
            sql.push_str(" IS NULL");
        } else {
            sql.push_str(" AND ");
            sql.push_str(&col("media_type"));
            sql.push_str(" = ?");
            values.push(media_type.clone());
        }
    }
}

fn searchable_text(msg: &MessageInsert) -> String {
    [
        msg.text.as_deref(),
        msg.display_text.as_deref(),
        msg.media_caption.as_deref(),
        msg.filename.as_deref(),
        msg.sender_name.as_deref(),
        msg.chat_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
}

fn escape_fts_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn map_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageRecord> {
    Ok(MessageRecord {
        rowid: row.get("rowid")?,
        chat_jid: row.get("chat_jid")?,
        chat_name: row.get("chat_name")?,
        msg_id: row.get("msg_id")?,
        sender_jid: row.get("sender_jid")?,
        sender_name: row.get("sender_name")?,
        ts: row.get("ts")?,
        from_me: row.get::<_, i64>("from_me")? != 0,
        text: row.get("text")?,
        display_text: row.get("display_text")?,
        media_type: row.get("media_type")?,
        media_caption: row.get("media_caption")?,
        filename: row.get("filename")?,
        mime_type: row.get("mime_type")?,
        direct_path: row.get("direct_path")?,
        file_length: row.get("file_length")?,
        local_path: row.get("local_path")?,
    })
}

fn map_contact(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContactRecord> {
    Ok(ContactRecord {
        jid: row.get("jid")?,
        lid: row.get("lid")?,
        phone_number: row.get("phone_number")?,
        name: row.get("name")?,
        full_name: row.get("full_name")?,
        first_name: row.get("first_name")?,
        username: row.get("username")?,
        push_name: row.get("push_name")?,
        source: row.get("source")?,
        from_full_sync: row.get::<_, i64>("from_full_sync")? != 0,
        updated_at: zero_as_none(row.get("updated_at")?),
        last_seen_ts: zero_as_none(row.get("last_seen_ts")?),
    })
}

fn zero_as_none(value: Option<i64>) -> Option<i64> {
    match value {
        Some(0) => None,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("index.db")).unwrap();
        (dir, store)
    }

    fn sample(id: &str, text: &str, ts: i64) -> MessageInsert {
        MessageInsert {
            chat_jid: "15551234567@s.whatsapp.net".into(),
            chat_name: Some("Test Chat".into()),
            msg_id: id.into(),
            sender_jid: Some("15551234567@s.whatsapp.net".into()),
            sender_name: Some("Tester".into()),
            ts,
            from_me: false,
            text: Some(text.into()),
            display_text: Some(text.into()),
            media_type: None,
            media_caption: None,
            filename: None,
            mime_type: None,
            direct_path: None,
            media_key: None,
            file_sha256: None,
            file_enc_sha256: None,
            file_length: None,
            raw_json: None,
        }
    }

    fn sample_media(id: &str, ts: i64) -> MessageInsert {
        MessageInsert {
            media_type: Some("image".into()),
            media_caption: Some("screenshot".into()),
            filename: Some("screen.jpg".into()),
            mime_type: Some("image/jpeg".into()),
            direct_path: Some("/v/t62.7118/test".into()),
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            file_length: Some(123),
            ..sample(id, "image row", ts)
        }
    }

    fn contact(jid: &str, name: &str) -> ContactUpsert {
        ContactUpsert {
            jid: jid.into(),
            lid: Some("111@lid".into()),
            phone_number: Some(jid.trim_end_matches("@s.whatsapp.net").into()),
            name: Some(name.into()),
            full_name: Some(name.into()),
            first_name: None,
            username: None,
            push_name: None,
            source: "whatsapp-app-state".into(),
            from_full_sync: true,
            updated_at: Some(100),
            last_seen_ts: None,
        }
    }

    #[test]
    fn stores_and_searches_messages() {
        let (_dir, mut store) = temp_store();
        store
            .upsert_message(&sample("a", "hello from rust", 10))
            .unwrap();
        store
            .upsert_message(&sample("b", "meeting tomorrow", 20))
            .unwrap();

        let filter = MessageFilter {
            limit: 10,
            ..Default::default()
        };
        let rows = store.search_messages("meeting", &filter).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].msg_id, "b");
    }

    #[test]
    fn search_handles_fts_punctuation() {
        let (_dir, mut store) = temp_store();
        store
            .upsert_message(&sample("a", "deploy failed: api/token", 10))
            .unwrap();

        let filter = MessageFilter {
            limit: 10,
            ..Default::default()
        };
        let rows = store.search_messages("failed: api/token", &filter).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].msg_id, "a");
    }

    #[test]
    fn context_returns_anchor_with_neighbors() {
        let (_dir, mut store) = temp_store();
        store.upsert_message(&sample("a", "one", 10)).unwrap();
        store.upsert_message(&sample("b", "two", 20)).unwrap();
        store.upsert_message(&sample("c", "three", 30)).unwrap();

        let rows = store
            .message_context("15551234567@s.whatsapp.net", "b", 1, 1)
            .unwrap();
        assert_eq!(
            rows.iter().map(|r| r.msg_id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn stores_media_download_metadata() {
        let (dir, mut store) = temp_store();
        store.upsert_message(&sample_media("m", 10)).unwrap();

        let media = store
            .media_message("15551234567@s.whatsapp.net", "m")
            .unwrap()
            .expect("media row should be downloadable");
        assert_eq!(media.media_type, "image");
        assert_eq!(media.file_length, 123);

        let path = dir.path().join("screen.jpg");
        store
            .mark_media_downloaded("15551234567@s.whatsapp.net", "m", &path)
            .unwrap();
        let row = store
            .show_message("15551234567@s.whatsapp.net", "m")
            .unwrap()
            .unwrap();
        assert_eq!(row.local_path.as_deref(), Some(path.to_str().unwrap()));
    }

    #[test]
    fn message_action_updates_keep_search_index_current() {
        let (_dir, mut store) = temp_store();
        store
            .upsert_message(&sample("edit", "before edit token", 10))
            .unwrap();

        assert!(
            store
                .update_message_text("15551234567@s.whatsapp.net", "edit", "after edit token")
                .unwrap()
        );

        let filter = MessageFilter {
            limit: 10,
            ..Default::default()
        };
        assert!(
            store
                .search_messages("before edit token", &filter)
                .unwrap()
                .is_empty()
        );
        let rows = store.search_messages("after edit token", &filter).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text.as_deref(), Some("after edit token"));
    }

    #[test]
    fn revoke_clears_searchable_media_fields() {
        let (_dir, mut store) = temp_store();
        store.upsert_message(&sample_media("m", 10)).unwrap();

        assert!(
            store
                .mark_message_revoked("15551234567@s.whatsapp.net", "m")
                .unwrap()
        );

        let row = store
            .show_message("15551234567@s.whatsapp.net", "m")
            .unwrap()
            .unwrap();
        assert_eq!(row.display_text.as_deref(), Some("Message revoked"));
        assert_eq!(row.media_type.as_deref(), Some("protocol"));
        assert!(row.direct_path.is_none());
        assert!(
            store
                .media_message("15551234567@s.whatsapp.net", "m")
                .unwrap()
                .is_none()
        );

        let filter = MessageFilter {
            limit: 10,
            ..Default::default()
        };
        assert!(
            store
                .search_messages("screenshot", &filter)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn delete_message_removes_row_and_fts_entry() {
        let (_dir, mut store) = temp_store();
        store
            .upsert_message(&sample("delete", "delete token", 10))
            .unwrap();

        assert!(
            store
                .delete_message("15551234567@s.whatsapp.net", "delete")
                .unwrap()
        );
        assert!(
            store
                .show_message("15551234567@s.whatsapp.net", "delete")
                .unwrap()
                .is_none()
        );

        let filter = MessageFilter {
            limit: 10,
            ..Default::default()
        };
        assert!(
            store
                .search_messages("delete token", &filter)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn analytics_counts_messages_by_direction_and_type() {
        let (_dir, mut store) = temp_store();
        let mut outbound = sample("out", "sent", 20);
        outbound.from_me = true;
        store.upsert_message(&sample("in", "received", 10)).unwrap();
        store.upsert_message(&outbound).unwrap();
        store.upsert_message(&sample_media("img", 30)).unwrap();

        let report = store.analytics(None).unwrap();
        assert_eq!(report.total_messages, 3);
        assert_eq!(report.inbound_messages, 2);
        assert_eq!(report.outbound_messages, 1);
        assert!(
            report
                .media_types
                .iter()
                .any(|row| row.media_type == "image")
        );
        assert!(
            report
                .media_types
                .iter()
                .any(|row| row.media_type == "text")
        );
    }

    #[test]
    fn stores_and_searches_contact_book() {
        let (_dir, mut store) = temp_store();
        store
            .upsert_contact(&contact("15551234567@s.whatsapp.net", "Devansh Riverline"))
            .unwrap();
        store
            .upsert_contact(&contact("15557654321@s.whatsapp.net", "Other Contact"))
            .unwrap();

        let rows = store
            .list_contact_book(Some("Riverline"), 10)
            .expect("contact search should work");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_deref(), Some("Devansh Riverline"));
        assert_eq!(rows[0].phone_number.as_deref(), Some("15551234567"));
    }
}
