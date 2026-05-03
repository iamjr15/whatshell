use anyhow::Result;
use comfy_table::{Cell, Table, presets::UTF8_FULL};
use serde::Serialize;

use crate::store::{ChatRecord, ContactRecord, MessageRecord};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

pub fn json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn json_response<T: Serialize>(value: T) -> Result<()> {
    json(&ApiResponse {
        success: true,
        data: value,
    })
}

pub fn print_messages(rows: &[MessageRecord]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "time", "chat", "sender", "dir", "type", "message", "id",
    ]);
    for row in rows {
        table.add_row(vec![
            Cell::new(format_ts(row.ts)),
            Cell::new(row.chat_name.as_deref().unwrap_or(&row.chat_jid)),
            Cell::new(
                row.sender_name
                    .as_deref()
                    .or(row.sender_jid.as_deref())
                    .unwrap_or(""),
            ),
            Cell::new(if row.from_me { "out" } else { "in" }),
            Cell::new(row.media_type.as_deref().unwrap_or("text")),
            Cell::new(truncate(row.display_text.as_deref().unwrap_or(""), 96)),
            Cell::new(truncate(&row.msg_id, 24)),
        ]);
    }
    println!("{table}");
}

pub fn print_chats(rows: &[ChatRecord]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["last", "kind", "name", "jid", "messages"]);
    for row in rows {
        table.add_row(vec![
            Cell::new(row.last_message_ts.map(format_ts).unwrap_or_default()),
            Cell::new(&row.kind),
            Cell::new(row.name.as_deref().unwrap_or("")),
            Cell::new(&row.jid),
            Cell::new(row.message_count),
        ]);
    }
    println!("{table}");
}

pub fn print_contacts(rows: &[ContactRecord]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["name", "push", "phone", "jid", "lid", "source"]);
    for row in rows {
        table.add_row(vec![
            Cell::new(
                row.name
                    .as_deref()
                    .or(row.full_name.as_deref())
                    .or(row.first_name.as_deref())
                    .or(row.username.as_deref())
                    .unwrap_or(""),
            ),
            Cell::new(row.push_name.as_deref().unwrap_or("")),
            Cell::new(row.phone_number.as_deref().unwrap_or("")),
            Cell::new(&row.jid),
            Cell::new(row.lid.as_deref().unwrap_or("")),
            Cell::new(&row.source),
        ]);
    }
    println!("{table}");
}

pub fn print_status(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref());
}

fn format_ts(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out = value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}
