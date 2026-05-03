use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use qrcode::QrCode;
use qrcode::render::unicode;
use serde::Serialize;
use tokio::sync::mpsc;
use wacore::iq::spec::IqSpec;
use wacore::request::InfoQuery;
use wacore_binary::jid::SERVER_JID;
use wacore_binary::node::{Node, NodeContent};
use whatsapp_rust::bot::Bot;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::store::SqliteStore;
use whatsapp_rust::sync_task::MajorSyncTask;
use whatsapp_rust::types::events::Event;
use whatsapp_rust::waproto::whatsapp as whatsapp_proto;
use whatsapp_rust::{
    BlocklistEntry, Client, GroupCreateOptions, GroupDescription, GroupParticipantOptions,
    GroupSubject, Jid, MemberAddMode, MembershipApprovalMode, RevokeType, StatusSendOptions,
    TokioRuntime,
};
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

use crate::cli::{
    AuthCommand, SendContact, SendFile, SendLocation, SendPoll, SendReact, SendText, SyncCommand,
};
use crate::config::AppConfig;
use crate::extract;
use crate::jid::{normalize_phone, parse_chat_or_phone};
use crate::output;
use crate::store::{
    ContactRecord, ContactUpsert, MediaRecord, MessageInsert, MessageRecord, Store,
};

use wacore::appstate::patch_decode::WAPatchName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Auth,
    Sync,
    Listen,
    Send,
}

#[derive(Debug)]
enum ControlEvent {
    Connected,
    PairSuccess,
    NeedsAuth,
    Stored,
    LoggedOut,
}

#[derive(Debug)]
enum IndexUpdate {
    Message(MessageInsert),
    Contact(ContactUpsert),
    Shutdown,
}

#[derive(Debug)]
struct StoreWriter {
    tx: std::sync::mpsc::Sender<IndexUpdate>,
    handle: std::thread::JoinHandle<Result<usize>>,
}

impl StoreWriter {
    fn start(config: AppConfig, stream_jsonl: bool) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<IndexUpdate>();
        let handle = std::thread::spawn(move || {
            let mut store = Store::open(&config.index_db())?;
            let mut count = 0usize;
            while let Ok(update) = rx.recv() {
                match update {
                    IndexUpdate::Message(message) => {
                        let rowid = store.upsert_message(&message)?;
                        count += 1;
                        if stream_jsonl {
                            let mut record = store
                                .show_message(&message.chat_jid, &message.msg_id)?
                                .ok_or_else(|| anyhow!("message inserted but not found"))?;
                            record.rowid = rowid;
                            println!("{}", serde_json::to_string(&record)?);
                        }
                    }
                    IndexUpdate::Contact(contact) => {
                        store.upsert_contact(&contact)?;
                        count += 1;
                    }
                    IndexUpdate::Shutdown => break,
                }
            }
            Ok(count)
        });
        Self { tx, handle }
    }

    fn sender(&self) -> std::sync::mpsc::Sender<IndexUpdate> {
        self.tx.clone()
    }

    fn shutdown(self) -> Result<usize> {
        let _ = self.tx.send(IndexUpdate::Shutdown);
        drop(self.tx);
        self.handle
            .join()
            .map_err(|_| anyhow!("store writer thread panicked"))?
    }
}

#[derive(Debug, Serialize)]
pub struct SendSummary {
    pub to: String,
    pub message_id: Option<String>,
    pub dry_run: bool,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_secret_hex: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ActionSummary {
    pub action: String,
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct MediaDownloadSummary {
    pub chat: String,
    pub message_id: String,
    pub path: String,
    pub bytes: usize,
    pub media_type: String,
}

#[derive(Debug, Serialize)]
pub struct ParticipantChangeSummary {
    pub jid: String,
    pub status: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BlocklistEntrySummary {
    pub jid: String,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct IsBlockedSummary {
    pub jid: String,
    pub blocked: bool,
}

#[derive(Debug, Serialize)]
pub struct ContactCheck {
    pub jid: String,
    pub is_registered: bool,
}

#[derive(Debug, Serialize)]
pub struct ContactInfoSummary {
    pub jid: String,
    pub lid: Option<String>,
    pub is_registered: bool,
    pub is_business: bool,
    pub status: Option<String>,
    pub picture_id: Option<u64>,
}

#[derive(Debug, Clone)]
struct UpdateBlocklistCompatSpec {
    jid: Jid,
    pn_jid: Option<Jid>,
    action: &'static str,
}

impl UpdateBlocklistCompatSpec {
    fn block(lid: Jid, pn: Jid) -> Self {
        Self {
            jid: lid,
            pn_jid: Some(pn),
            action: "block",
        }
    }

    fn unblock(jid: Jid) -> Self {
        Self {
            jid,
            pn_jid: None,
            action: "unblock",
        }
    }
}

impl IqSpec for UpdateBlocklistCompatSpec {
    type Response = ();

    fn build_iq(&self) -> InfoQuery<'static> {
        let mut item = whatsapp_rust::NodeBuilder::new("item")
            .attr("jid", self.jid.clone())
            .attr("action", self.action);
        if let Some(pn_jid) = &self.pn_jid {
            item = item.attr("pn_jid", pn_jid.clone());
        }
        InfoQuery::set(
            "blocklist",
            Jid::new("", SERVER_JID),
            Some(NodeContent::Nodes(vec![item.build()])),
        )
    }

    fn parse_response(&self, _response: &Node) -> Result<Self::Response> {
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ContactSyncSummary {
    pub source: String,
    pub collection: String,
    pub full_sync: bool,
    pub indexed_updates: usize,
}

#[derive(Debug, Serialize)]
pub struct GroupSummary {
    pub jid: String,
    pub subject: String,
    pub description: Option<String>,
    pub size: Option<u32>,
    pub participant_count: usize,
    pub is_locked: bool,
    pub is_announcement: bool,
    pub membership_approval: bool,
    pub participants: Vec<GroupParticipantSummary>,
}

#[derive(Debug, Serialize)]
pub struct GroupParticipantSummary {
    pub jid: String,
    pub phone_number: Option<String>,
    pub is_admin: bool,
}

pub async fn auth(config: AppConfig, args: AuthCommand) -> Result<()> {
    config.ensure_dirs()?;
    let writer = StoreWriter::start(config.clone(), false);
    let (control_tx, mut control_rx) = mpsc::unbounded_channel();
    let (mut bot, client) = build_bot(
        &config,
        RunMode::Auth,
        args.phone,
        args.code,
        Some(writer.sender()),
        control_tx,
    )
    .await?;
    let handle = bot.run().await?;
    let idle_after = Duration::from_secs(args.idle_exit);
    let mut connected = false;
    let mut last_activity = Instant::now();

    loop {
        tokio::select! {
            event = control_rx.recv() => {
                match event {
                    Some(ControlEvent::Connected | ControlEvent::PairSuccess) => {
                        connected = true;
                        last_activity = Instant::now();
                    }
                    Some(ControlEvent::Stored) => {
                        last_activity = Instant::now();
                    }
                    Some(ControlEvent::LoggedOut) => return Err(anyhow!("WhatsApp session logged out")),
                    Some(ControlEvent::NeedsAuth) | None => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if connected && !args.follow && last_activity.elapsed() >= idle_after {
                    client.disconnect().await;
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                client.disconnect().await;
                break;
            }
        }
    }

    let _ = handle.await;
    drop(client);
    drop(bot);
    let stored = writer.shutdown()?;
    output::print_status(format!("auth complete; indexed {stored} messages"));
    Ok(())
}

pub async fn sync(config: AppConfig, args: SyncCommand) -> Result<()> {
    run_capture(
        config,
        RunMode::Sync,
        args.stream_jsonl,
        args.once,
        args.follow || !args.once,
        args.idle_exit,
    )
    .await
}

pub async fn listen(config: AppConfig, stream_jsonl: bool) -> Result<()> {
    run_capture(config, RunMode::Listen, stream_jsonl, false, true, 0).await
}

async fn run_capture(
    config: AppConfig,
    mode: RunMode,
    stream_jsonl: bool,
    once: bool,
    follow: bool,
    idle_exit_secs: u64,
) -> Result<()> {
    config.ensure_dirs()?;
    let writer = StoreWriter::start(config.clone(), stream_jsonl);
    let (control_tx, mut control_rx) = mpsc::unbounded_channel();
    let (mut bot, client) =
        build_bot(&config, mode, None, None, Some(writer.sender()), control_tx).await?;
    let handle = bot.run().await?;
    let idle_after = Duration::from_secs(idle_exit_secs.max(1));
    let mut connected = false;
    let mut last_activity = Instant::now();

    loop {
        tokio::select! {
            event = control_rx.recv() => {
                match event {
                    Some(ControlEvent::Connected) => {
                        connected = true;
                        last_activity = Instant::now();
                        if once && idle_exit_secs == 0 {
                            client.disconnect().await;
                            break;
                        }
                    }
                    Some(ControlEvent::Stored) => last_activity = Instant::now(),
                    Some(ControlEvent::NeedsAuth) => {
                        client.disconnect().await;
                        return Err(anyhow!("not authenticated; run `whatshell auth` first"));
                    }
                    Some(ControlEvent::LoggedOut) => return Err(anyhow!("WhatsApp session logged out; run `whatshell auth`")),
                    Some(ControlEvent::PairSuccess) | None => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if connected && !follow && last_activity.elapsed() >= idle_after {
                    client.disconnect().await;
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                client.disconnect().await;
                break;
            }
        }
    }

    let _ = handle.await;
    drop(client);
    drop(bot);
    let stored = writer.shutdown()?;
    output::print_status(format!("sync complete; indexed {stored} messages"));
    Ok(())
}

pub async fn send_text(config: AppConfig, args: SendText) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(&args.to)?;
    let reply_sender = args
        .reply_sender
        .as_deref()
        .map(parse_chat_or_phone)
        .transpose()?;
    let message = build_text_message(
        &to,
        &args.message,
        args.reply_to.as_deref(),
        reply_sender.as_ref(),
    );
    if args.dry_run {
        return Ok(SendSummary {
            to: to.to_string(),
            message_id: None,
            dry_run: true,
            kind: "text".into(),
            message_secret_hex: None,
        });
    }
    let (client, handle) = connect_for_send(config.clone()).await?;
    let sent = client
        .send_message(to.clone(), message.clone())
        .await
        .context("send text message")?;
    index_outgoing(&config, &to, &sent, &message)?;
    client.disconnect().await;
    let _ = handle.await;
    drop(client);
    Ok(SendSummary {
        to: to.to_string(),
        message_id: Some(sent),
        dry_run: false,
        kind: "text".into(),
        message_secret_hex: None,
    })
}

pub async fn send_react(config: AppConfig, args: SendReact) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(&args.to)?;
    let participant = args
        .sender
        .as_deref()
        .map(parse_chat_or_phone)
        .transpose()?;
    let message = whatsapp_proto::Message {
        reaction_message: Some(whatsapp_proto::message::ReactionMessage {
            key: Some(whatsapp_proto::MessageKey {
                remote_jid: Some(to.to_string()),
                from_me: Some(args.from_me),
                id: Some(args.id.clone()),
                participant: participant.map(|jid| jid.to_string()),
            }),
            text: Some(args.reaction.clone()),
            sender_timestamp_ms: Some(chrono::Utc::now().timestamp_millis()),
            ..Default::default()
        }),
        ..Default::default()
    };
    if args.dry_run {
        return Ok(SendSummary {
            to: to.to_string(),
            message_id: None,
            dry_run: true,
            kind: "reaction".into(),
            message_secret_hex: None,
        });
    }
    let (client, handle) = connect_for_send(config.clone()).await?;
    let sent = client
        .send_message(to.clone(), message.clone())
        .await
        .context("send reaction")?;
    index_outgoing(&config, &to, &sent, &message)?;
    client.disconnect().await;
    let _ = handle.await;
    drop(client);
    Ok(SendSummary {
        to: to.to_string(),
        message_id: Some(sent),
        dry_run: false,
        kind: "reaction".into(),
        message_secret_hex: None,
    })
}

pub async fn send_file(config: AppConfig, args: SendFile) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(&args.to)?;
    let mime = args.mime.clone().unwrap_or_else(|| {
        mime_guess::from_path(&args.file)
            .first_or_octet_stream()
            .essence_str()
            .to_string()
    });
    let kind = media_type_from_mime(&mime);
    if args.dry_run {
        return Ok(SendSummary {
            to: to.to_string(),
            message_id: None,
            dry_run: true,
            kind: kind_name(kind).into(),
            message_secret_hex: None,
        });
    }

    let data = tokio::fs::read(&args.file)
        .await
        .with_context(|| format!("read {}", args.file.display()))?;
    let (client, handle) = connect_for_send(config.clone()).await?;
    let upload = client
        .upload(data, kind)
        .await
        .with_context(|| format!("upload {}", args.file.display()))?;
    let filename = args.filename.or_else(|| {
        args.file
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToOwned::to_owned)
    });
    let message = build_media_message(kind, &mime, &upload, args.caption, filename);
    let sent = client
        .send_message(to.clone(), message.clone())
        .await
        .context("send file message")?;
    index_outgoing(&config, &to, &sent, &message)?;
    client.disconnect().await;
    let _ = handle.await;
    drop(client);
    Ok(SendSummary {
        to: to.to_string(),
        message_id: Some(sent),
        dry_run: false,
        kind: kind_name(kind).into(),
        message_secret_hex: None,
    })
}

pub async fn send_poll(config: AppConfig, args: SendPoll) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(&args.to)?;
    validate_poll(&args.options, args.selectable_count)?;
    if args.dry_run {
        return Ok(SendSummary {
            to: to.to_string(),
            message_id: None,
            dry_run: true,
            kind: "poll".into(),
            message_secret_hex: None,
        });
    }
    let (client, handle) = connect_for_send(config.clone()).await?;
    let (message_id, secret) = client
        .polls()
        .create(&to, &args.question, &args.options, args.selectable_count)
        .await?;
    index_outgoing_preview(&config, &to, &message_id, "poll", &args.question)?;
    finish_live_command(client, handle).await;
    Ok(SendSummary {
        to: to.to_string(),
        message_id: Some(message_id),
        dry_run: false,
        kind: "poll".into(),
        message_secret_hex: Some(hex::encode(secret)),
    })
}

pub async fn send_location(config: AppConfig, args: SendLocation) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(&args.to)?;
    let message = whatsapp_proto::Message {
        location_message: Some(Box::new(whatsapp_proto::message::LocationMessage {
            degrees_latitude: Some(args.latitude),
            degrees_longitude: Some(args.longitude),
            name: args.name,
            address: args.address,
            url: args.url,
            ..Default::default()
        })),
        ..Default::default()
    };
    if args.dry_run {
        return Ok(SendSummary {
            to: to.to_string(),
            message_id: None,
            dry_run: true,
            kind: "location".into(),
            message_secret_hex: None,
        });
    }
    let (client, handle) = connect_for_send(config.clone()).await?;
    let sent = client.send_message(to.clone(), message.clone()).await?;
    index_outgoing(&config, &to, &sent, &message)?;
    finish_live_command(client, handle).await;
    Ok(SendSummary {
        to: to.to_string(),
        message_id: Some(sent),
        dry_run: false,
        kind: "location".into(),
        message_secret_hex: None,
    })
}

pub async fn send_contact(config: AppConfig, args: SendContact) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(&args.to)?;
    let normalized_phone = normalize_phone(&args.phone)?;
    let vcard = args.vcard.unwrap_or_else(|| {
        format!(
            "BEGIN:VCARD\nVERSION:3.0\nFN:{}\nTEL;type=CELL;waid={}:{}\nEND:VCARD",
            args.name, normalized_phone, normalized_phone
        )
    });
    let message = whatsapp_proto::Message {
        contact_message: Some(Box::new(whatsapp_proto::message::ContactMessage {
            display_name: Some(args.name),
            vcard: Some(vcard),
            ..Default::default()
        })),
        ..Default::default()
    };
    if args.dry_run {
        return Ok(SendSummary {
            to: to.to_string(),
            message_id: None,
            dry_run: true,
            kind: "contact".into(),
            message_secret_hex: None,
        });
    }
    let (client, handle) = connect_for_send(config.clone()).await?;
    let sent = client.send_message(to.clone(), message.clone()).await?;
    index_outgoing(&config, &to, &sent, &message)?;
    finish_live_command(client, handle).await;
    Ok(SendSummary {
        to: to.to_string(),
        message_id: Some(sent),
        dry_run: false,
        kind: "contact".into(),
        message_secret_hex: None,
    })
}

pub async fn message_edit(
    config: AppConfig,
    chat: &str,
    id: &str,
    text: &str,
    dry_run: bool,
) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(chat)?;
    if dry_run {
        return Ok(SendSummary {
            to: to.to_string(),
            message_id: Some(id.to_string()),
            dry_run: true,
            kind: "edit".into(),
            message_secret_hex: None,
        });
    }
    let index_db = config.index_db();
    let (client, handle) = connect_for_send(config).await?;
    let edited = client
        .edit_message(to.clone(), id, build_text_message(&to, text, None, None))
        .await?;
    finish_live_command(client, handle).await;
    Store::open(&index_db)?.update_message_text(&to.to_string(), id, text)?;
    Ok(SendSummary {
        to: to.to_string(),
        message_id: Some(edited),
        dry_run: false,
        kind: "edit".into(),
        message_secret_hex: None,
    })
}

pub async fn message_revoke(
    config: AppConfig,
    chat: &str,
    id: &str,
    admin_sender: Option<&str>,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let to = parse_chat_or_phone(chat)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run revoke".into(),
            target: format!("{to}/{id}"),
        });
    }
    let revoke_type = match admin_sender {
        Some(sender) => RevokeType::Admin {
            original_sender: parse_chat_or_phone(sender)?,
        },
        None => RevokeType::Sender,
    };
    let index_db = config.index_db();
    let (client, handle) = connect_for_send(config).await?;
    client
        .revoke_message(to.clone(), id.to_string(), revoke_type)
        .await?;
    finish_live_command(client, handle).await;
    Store::open(&index_db)?.mark_message_revoked(&to.to_string(), id)?;
    Ok(ActionSummary {
        action: "revoke".into(),
        target: format!("{to}/{id}"),
    })
}

pub async fn message_delete_for_me(
    config: AppConfig,
    row: &MessageRecord,
    delete_media: bool,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let chat = parse_chat_or_phone(&row.chat_jid)?;
    let participant = participant_from_row(row)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run delete-for-me".into(),
            target: format!("{}/{}", row.chat_jid, row.msg_id),
        });
    }
    let index_db = config.index_db();
    let (client, handle) = connect_for_send(config).await?;
    client
        .chat_actions()
        .delete_message_for_me(
            &chat,
            participant.as_ref(),
            &row.msg_id,
            row.from_me,
            delete_media,
            Some(row.ts),
        )
        .await?;
    finish_live_command(client, handle).await;
    Store::open(&index_db)?.delete_message(&row.chat_jid, &row.msg_id)?;
    Ok(ActionSummary {
        action: "delete-for-me".into(),
        target: format!("{}/{}", row.chat_jid, row.msg_id),
    })
}

pub async fn message_star(
    config: AppConfig,
    row: &MessageRecord,
    starred: bool,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let chat = parse_chat_or_phone(&row.chat_jid)?;
    let participant = participant_from_row(row)?;
    if dry_run {
        return Ok(ActionSummary {
            action: format!("dry-run {}", if starred { "star" } else { "unstar" }),
            target: format!("{}/{}", row.chat_jid, row.msg_id),
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    if starred {
        client
            .chat_actions()
            .star_message(&chat, participant.as_ref(), &row.msg_id, row.from_me)
            .await?;
    } else {
        client
            .chat_actions()
            .unstar_message(&chat, participant.as_ref(), &row.msg_id, row.from_me)
            .await?;
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if starred { "star" } else { "unstar" }.into(),
        target: format!("{}/{}", row.chat_jid, row.msg_id),
    })
}

pub async fn media_download(
    config: AppConfig,
    media: MediaRecord,
    output: Option<PathBuf>,
    overwrite: bool,
) -> Result<MediaDownloadSummary> {
    ensure_can_write(&config)?;
    let media_type = media_type_from_store(&media.media_type)?;
    let output_path = resolve_media_output(&config, &media, output)?;
    if output_path.exists() && !overwrite {
        return Err(anyhow!(
            "{} already exists; pass --overwrite to replace it",
            output_path.display()
        ));
    }
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let (client, handle) = connect_for_send(config.clone()).await?;
    let data = client
        .download_from_params(
            &media.direct_path,
            &media.media_key,
            &media.file_sha256,
            &media.file_enc_sha256,
            media.file_length as u64,
            media_type,
        )
        .await?;
    finish_live_command(client, handle).await;
    tokio::fs::write(&output_path, &data).await?;
    Store::open(&config.index_db())?.mark_media_downloaded(
        &media.chat_jid,
        &media.msg_id,
        &output_path,
    )?;
    Ok(MediaDownloadSummary {
        chat: media.chat_jid,
        message_id: media.msg_id,
        path: output_path.display().to_string(),
        bytes: data.len(),
        media_type: media.media_type,
    })
}

pub async fn logout(config: AppConfig) -> Result<()> {
    ensure_can_write(&config)?;
    remove_session_files(&config)?;
    Ok(())
}

pub async fn chat_archive(config: AppConfig, chat: &str, archived: bool) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(chat)?;
    let (client, handle) = connect_for_send(config).await?;
    if archived {
        client.chat_actions().archive_chat(&jid, None).await?;
    } else {
        client.chat_actions().unarchive_chat(&jid, None).await?;
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if archived { "archive" } else { "unarchive" }.into(),
        target: jid.to_string(),
    })
}

pub async fn chat_pin(config: AppConfig, chat: &str, pinned: bool) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(chat)?;
    let (client, handle) = connect_for_send(config).await?;
    if pinned {
        client.chat_actions().pin_chat(&jid).await?;
    } else {
        client.chat_actions().unpin_chat(&jid).await?;
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if pinned { "pin" } else { "unpin" }.into(),
        target: jid.to_string(),
    })
}

pub async fn chat_mute(config: AppConfig, chat: &str, muted: bool) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(chat)?;
    let (client, handle) = connect_for_send(config).await?;
    if muted {
        client.chat_actions().mute_chat(&jid).await?;
    } else {
        client.chat_actions().unmute_chat(&jid).await?;
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if muted { "mute" } else { "unmute" }.into(),
        target: jid.to_string(),
    })
}

pub async fn chat_mark_read(config: AppConfig, chat: &str, read: bool) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(chat)?;
    let (client, handle) = connect_for_send(config).await?;
    client
        .chat_actions()
        .mark_chat_as_read(&jid, read, None)
        .await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if read { "mark-read" } else { "mark-unread" }.into(),
        target: jid.to_string(),
    })
}

pub async fn chat_delete(
    config: AppConfig,
    chat: &str,
    delete_media: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(chat)?;
    let (client, handle) = connect_for_send(config).await?;
    client
        .chat_actions()
        .delete_chat(&jid, delete_media, None)
        .await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "delete-chat".into(),
        target: jid.to_string(),
    })
}

pub async fn presence_chatstate(
    config: AppConfig,
    chat: &str,
    state: &str,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(chat)?;
    let (client, handle) = connect_for_send(config).await?;
    match state {
        "typing" => client.chatstate().send_composing(&jid).await?,
        "recording" => client.chatstate().send_recording(&jid).await?,
        "paused" => client.chatstate().send_paused(&jid).await?,
        _ => return Err(anyhow!("unknown chat state {state}")),
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: state.into(),
        target: jid.to_string(),
    })
}

pub async fn presence_set(config: AppConfig, available: bool) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let (client, handle) = connect_for_send(config).await?;
    if available {
        client.presence().set_available().await?;
    } else {
        client.presence().set_unavailable().await?;
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if available { "online" } else { "offline" }.into(),
        target: "self".into(),
    })
}

pub async fn check_connection(config: AppConfig) -> Result<()> {
    let (client, handle) = connect_for_send(config).await?;
    finish_live_command(client, handle).await;
    Ok(())
}

pub async fn contacts_check(config: AppConfig, phones: Vec<String>) -> Result<Vec<ContactCheck>> {
    let phones = normalize_phones(phones)?;
    let refs = phones.iter().map(String::as_str).collect::<Vec<_>>();
    let (client, handle) = connect_for_send(config).await?;
    let rows = client.contacts().is_on_whatsapp(&refs).await?;
    finish_live_command(client, handle).await;
    Ok(rows
        .into_iter()
        .map(|row| ContactCheck {
            jid: row.jid.to_string(),
            is_registered: row.is_registered,
        })
        .collect())
}

pub async fn contacts_info(
    config: AppConfig,
    phones: Vec<String>,
) -> Result<Vec<ContactInfoSummary>> {
    let phones = normalize_phones(phones)?;
    let refs = phones.iter().map(String::as_str).collect::<Vec<_>>();
    let (client, handle) = connect_for_send(config).await?;
    let rows = client.contacts().get_info(&refs).await?;
    finish_live_command(client, handle).await;
    Ok(rows
        .into_iter()
        .map(|row| ContactInfoSummary {
            jid: row.jid.to_string(),
            lid: row.lid.map(|jid| jid.to_string()),
            is_registered: row.is_registered,
            is_business: row.is_business,
            status: row.status,
            picture_id: row.picture_id,
        })
        .collect())
}

pub async fn contacts_sync(config: AppConfig) -> Result<ContactSyncSummary> {
    let writer = StoreWriter::start(config.clone(), false);
    let (client, handle) = connect_for_live_command(config.clone(), Some(writer.sender())).await?;
    let sync_timeout = Duration::from_secs(config.timeout_secs.max(30));
    let sync = tokio::time::timeout(
        sync_timeout,
        client.process_sync_task(MajorSyncTask::AppStateSync {
            name: WAPatchName::CriticalUnblockLow,
            full_sync: true,
        }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    finish_live_command(client, handle).await;
    let indexed_updates = writer.shutdown()?;
    sync.map_err(|_| {
        anyhow!(
            "timed out syncing WhatsApp contacts after {} seconds",
            sync_timeout.as_secs()
        )
    })?;

    Ok(ContactSyncSummary {
        source: "whatsapp-app-state".into(),
        collection: WAPatchName::CriticalUnblockLow.as_str().into(),
        full_sync: true,
        indexed_updates,
    })
}

pub async fn groups_list(config: AppConfig, query: Option<String>) -> Result<Vec<GroupSummary>> {
    let (client, handle) = connect_for_send(config).await?;
    let groups = client.groups().get_participating().await?;
    finish_live_command(client, handle).await;
    let query = query.map(|q| q.to_lowercase());
    let mut rows = groups
        .into_values()
        .map(group_summary)
        .filter(|group| {
            query.as_ref().is_none_or(|q| {
                group.jid.to_lowercase().contains(q) || group.subject.to_lowercase().contains(q)
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a.subject.cmp(&b.subject));
    Ok(rows)
}

pub async fn groups_info(config: AppConfig, jid: &str) -> Result<GroupSummary> {
    let jid = parse_chat_or_phone(jid)?;
    let (client, handle) = connect_for_send(config).await?;
    let group = client.groups().get_metadata(&jid).await?;
    finish_live_command(client, handle).await;
    Ok(group_summary(group))
}

pub async fn groups_invite_link(
    config: AppConfig,
    jid: &str,
    reset: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    let (client, handle) = connect_for_send(config).await?;
    let link = client.groups().get_invite_link(&jid, reset).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if reset {
            "reset-invite-link".into()
        } else {
            "invite-link".into()
        },
        target: link,
    })
}

pub async fn groups_invite_info(config: AppConfig, code: &str) -> Result<GroupSummary> {
    let (client, handle) = connect_for_send(config).await?;
    let group = client.groups().get_invite_info(code).await?;
    finish_live_command(client, handle).await;
    Ok(group_summary(group))
}

pub async fn groups_leave(config: AppConfig, jid: &str) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    let (client, handle) = connect_for_send(config).await?;
    client.groups().leave(&jid).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "leave-group".into(),
        target: jid.to_string(),
    })
}

pub async fn groups_create(
    config: AppConfig,
    subject: &str,
    participants: &[String],
    admins_only_add: bool,
    approval: bool,
    ephemeral_seconds: u32,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let participant_options = parse_group_participants(participants)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run create-group".into(),
            target: subject.to_string(),
        });
    }
    let add_mode = if admins_only_add {
        MemberAddMode::AdminAdd
    } else {
        MemberAddMode::AllMemberAdd
    };
    let approval_mode = if approval {
        MembershipApprovalMode::On
    } else {
        MembershipApprovalMode::Off
    };
    let options = GroupCreateOptions::new(subject)
        .with_participants(participant_options)
        .with_member_add_mode(add_mode)
        .with_membership_approval_mode(approval_mode)
        .with_ephemeral_expiration(ephemeral_seconds);
    let (client, handle) = connect_for_send(config).await?;
    let created = client.groups().create_group(options).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "create-group".into(),
        target: created.gid.to_string(),
    })
}

pub async fn groups_set_subject(
    config: AppConfig,
    jid: &str,
    subject: &str,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    let subject = GroupSubject::new(subject)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run set-subject".into(),
            target: jid.to_string(),
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    client.groups().set_subject(&jid, subject).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "set-subject".into(),
        target: jid.to_string(),
    })
}

pub async fn groups_set_description(
    config: AppConfig,
    jid: &str,
    description: Option<String>,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    let description = description
        .map(|text| GroupDescription::new(&text))
        .transpose()?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run set-description".into(),
            target: jid.to_string(),
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    let previous_id = client.groups().get_metadata(&jid).await?.description_id;
    if description.is_none() && previous_id.is_none() {
        finish_live_command(client, handle).await;
        return Ok(ActionSummary {
            action: "set-description".into(),
            target: jid.to_string(),
        });
    }
    client
        .groups()
        .set_description(&jid, description, previous_id)
        .await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "set-description".into(),
        target: jid.to_string(),
    })
}

pub async fn groups_participants(
    config: AppConfig,
    jid: &str,
    participants: &[String],
    action: &str,
    dry_run: bool,
) -> Result<Vec<ParticipantChangeSummary>> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    let participants = parse_jids(participants)?;
    if dry_run {
        return Ok(participants
            .into_iter()
            .map(|jid| ParticipantChangeSummary {
                jid: jid.to_string(),
                status: Some(format!("dry-run {action}")),
                error: None,
            })
            .collect());
    }
    let (client, handle) = connect_for_send(config).await?;
    let rows = match action {
        "add" => {
            client
                .groups()
                .add_participants(&jid, &participants)
                .await?
        }
        "remove" => {
            client
                .groups()
                .remove_participants(&jid, &participants)
                .await?
        }
        "approve" => {
            client
                .groups()
                .approve_membership_requests(&jid, &participants)
                .await?
        }
        "reject" => {
            client
                .groups()
                .reject_membership_requests(&jid, &participants)
                .await?
        }
        "promote" => {
            client
                .groups()
                .promote_participants(&jid, &participants)
                .await?;
            participants
                .into_iter()
                .map(|jid| whatsapp_rust::ParticipantChangeResponse {
                    jid,
                    status: Some("200".into()),
                    error: None,
                })
                .collect()
        }
        "demote" => {
            client
                .groups()
                .demote_participants(&jid, &participants)
                .await?;
            participants
                .into_iter()
                .map(|jid| whatsapp_rust::ParticipantChangeResponse {
                    jid,
                    status: Some("200".into()),
                    error: None,
                })
                .collect()
        }
        _ => return Err(anyhow!("unknown group participant action {action}")),
    };
    finish_live_command(client, handle).await;
    Ok(rows.into_iter().map(participant_change_summary).collect())
}

pub async fn groups_setting(
    config: AppConfig,
    jid: &str,
    action: &str,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    if dry_run {
        return Ok(ActionSummary {
            action: format!("dry-run {action}"),
            target: jid.to_string(),
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    match action {
        "lock" => client.groups().set_locked(&jid, true).await?,
        "unlock" => client.groups().set_locked(&jid, false).await?,
        "announce" => client.groups().set_announce(&jid, true).await?,
        "unannounce" => client.groups().set_announce(&jid, false).await?,
        _ => return Err(anyhow!("unknown group setting {action}")),
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: action.into(),
        target: jid.to_string(),
    })
}

pub async fn groups_set_ephemeral(
    config: AppConfig,
    jid: &str,
    seconds: u32,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run ephemeral".into(),
            target: jid.to_string(),
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    client.groups().set_ephemeral(&jid, seconds).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: format!("ephemeral={seconds}"),
        target: jid.to_string(),
    })
}

pub async fn groups_set_approval(
    config: AppConfig,
    jid: &str,
    on: bool,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run approval".into(),
            target: jid.to_string(),
        });
    }
    let mode = if on {
        MembershipApprovalMode::On
    } else {
        MembershipApprovalMode::Off
    };
    let (client, handle) = connect_for_send(config).await?;
    client.groups().set_membership_approval(&jid, mode).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if on { "approval-on" } else { "approval-off" }.into(),
        target: jid.to_string(),
    })
}

pub async fn groups_set_member_add(
    config: AppConfig,
    jid: &str,
    admins_only: bool,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(jid)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run member-add".into(),
            target: jid.to_string(),
        });
    }
    let mode = if admins_only {
        MemberAddMode::AdminAdd
    } else {
        MemberAddMode::AllMemberAdd
    };
    let (client, handle) = connect_for_send(config).await?;
    client.groups().set_member_add_mode(&jid, mode).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if admins_only {
            "member-add-admins"
        } else {
            "member-add-everyone"
        }
        .into(),
        target: jid.to_string(),
    })
}

pub async fn groups_requests(
    config: AppConfig,
    jid: &str,
) -> Result<Vec<ParticipantChangeSummary>> {
    let jid = parse_chat_or_phone(jid)?;
    let (client, handle) = connect_for_send(config).await?;
    let rows = client.groups().get_membership_requests(&jid).await?;
    finish_live_command(client, handle).await;
    Ok(rows
        .into_iter()
        .map(|row| ParticipantChangeSummary {
            jid: row.jid.to_string(),
            status: row.request_time.map(|ts| ts.to_string()),
            error: None,
        })
        .collect())
}

pub async fn profile_set_name(
    config: AppConfig,
    name: &str,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run set-name".into(),
            target: "self".into(),
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    client.profile().set_push_name(name).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "set-name".into(),
        target: "self".into(),
    })
}

pub async fn profile_set_about(
    config: AppConfig,
    text: &str,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run set-about".into(),
            target: "self".into(),
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    client.profile().set_status_text(text).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "set-about".into(),
        target: "self".into(),
    })
}

pub async fn profile_set_picture(
    config: AppConfig,
    file: &Path,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    if dry_run {
        return Ok(ActionSummary {
            action: "dry-run set-picture".into(),
            target: file.display().to_string(),
        });
    }
    let data = tokio::fs::read(file)
        .await
        .with_context(|| format!("read {}", file.display()))?;
    let (client, handle) = connect_for_send(config).await?;
    client.profile().set_profile_picture(data).await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "set-picture".into(),
        target: "self".into(),
    })
}

pub async fn profile_remove_picture(config: AppConfig) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let (client, handle) = connect_for_send(config).await?;
    client.profile().remove_profile_picture().await?;
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: "remove-picture".into(),
        target: "self".into(),
    })
}

#[derive(Debug, Clone, Default)]
struct BlocklistJidMapping {
    lid: Option<Jid>,
    pn: Option<Jid>,
}

fn lookup_blocklist_mapping(config: &AppConfig, jid: &Jid) -> Option<BlocklistJidMapping> {
    let store = Store::open(&config.index_db()).ok()?;
    let mut rows = store.list_contact_book(Some(&jid.to_string()), 10).ok()?;
    if rows.is_empty() && jid.is_pn() {
        rows = store.list_contact_book(Some(&jid.user), 10).ok()?;
    }
    rows.into_iter().find_map(blocklist_mapping_from_contact)
}

fn blocklist_mapping_from_contact(row: ContactRecord) -> Option<BlocklistJidMapping> {
    let lid = row
        .lid
        .as_deref()
        .and_then(|value| value.parse::<Jid>().ok())
        .filter(Jid::is_lid);
    let pn = row.jid.parse::<Jid>().ok().filter(Jid::is_pn).or_else(|| {
        row.phone_number.as_deref().and_then(|phone| {
            if phone.contains('@') {
                phone.parse::<Jid>().ok().filter(Jid::is_pn)
            } else {
                normalize_phone(phone).ok().map(Jid::pn)
            }
        })
    });
    if lid.is_none() && pn.is_none() {
        None
    } else {
        Some(BlocklistJidMapping { lid, pn })
    }
}

async fn resolve_block_jids(
    client: &Client,
    jid: &Jid,
    mapping: Option<BlocklistJidMapping>,
) -> Result<(Jid, Jid)> {
    let pn_jid = if jid.is_pn() {
        Some(jid.clone())
    } else {
        mapping.as_ref().and_then(|item| item.pn.clone())
    };
    let mut lid_jid = if jid.is_lid() {
        Some(jid.clone())
    } else {
        mapping.as_ref().and_then(|item| item.lid.clone())
    };

    if lid_jid.is_none() && jid.is_pn() {
        let rows = client.contacts().get_info(&[jid.user.as_str()]).await?;
        lid_jid = rows.into_iter().find_map(|row| row.lid);
    }

    let lid_jid = lid_jid.ok_or_else(|| {
        anyhow!("blocking requires the contact LID; run `whatshell contacts sync` and retry")
    })?;
    let pn_jid = pn_jid.ok_or_else(|| {
        anyhow!(
            "blocking requires the contact phone JID; run `whatshell contacts sync` or pass the phone-number JID"
        )
    })?;
    Ok((lid_jid, pn_jid))
}

fn preferred_blocklist_jid(jid: &Jid, mapping: Option<BlocklistJidMapping>) -> Option<Jid> {
    if jid.is_lid() {
        Some(jid.clone())
    } else {
        mapping.and_then(|item| item.lid)
    }
}

pub async fn blocking_list(config: AppConfig) -> Result<Vec<BlocklistEntrySummary>> {
    let (client, handle) = connect_for_send(config).await?;
    let rows = client.blocking().get_blocklist().await?;
    finish_live_command(client, handle).await;
    Ok(rows.into_iter().map(blocklist_entry_summary).collect())
}

pub async fn blocking_set(
    config: AppConfig,
    contact: &str,
    blocked: bool,
    dry_run: bool,
) -> Result<ActionSummary> {
    ensure_can_write(&config)?;
    let jid = parse_chat_or_phone(contact)?;
    if dry_run {
        return Ok(ActionSummary {
            action: format!("dry-run {}", if blocked { "block" } else { "unblock" }),
            target: jid.to_string(),
        });
    }
    let mapping = lookup_blocklist_mapping(&config, &jid);
    let (client, handle) = connect_for_send(config).await?;
    if blocked {
        let (lid_jid, pn_jid) = resolve_block_jids(&client, &jid, mapping).await?;
        client
            .execute(UpdateBlocklistCompatSpec::block(lid_jid, pn_jid))
            .await?;
    } else {
        let target = preferred_blocklist_jid(&jid, mapping).unwrap_or_else(|| jid.clone());
        client
            .execute(UpdateBlocklistCompatSpec::unblock(target))
            .await?;
    }
    finish_live_command(client, handle).await;
    Ok(ActionSummary {
        action: if blocked { "block" } else { "unblock" }.into(),
        target: jid.to_string(),
    })
}

pub async fn blocking_is_blocked(config: AppConfig, contact: &str) -> Result<IsBlockedSummary> {
    let jid = parse_chat_or_phone(contact)?;
    let mapping = lookup_blocklist_mapping(&config, &jid);
    let target = preferred_blocklist_jid(&jid, mapping).unwrap_or_else(|| jid.clone());
    let (client, handle) = connect_for_send(config).await?;
    let blocked = client.blocking().is_blocked(&target).await?;
    finish_live_command(client, handle).await;
    Ok(IsBlockedSummary {
        jid: jid.to_string(),
        blocked,
    })
}

pub async fn status_text(
    config: AppConfig,
    text: &str,
    recipients: &[String],
    background: &str,
    font: i32,
    dry_run: bool,
) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let recipients = parse_jids(recipients)?;
    if dry_run {
        return Ok(SendSummary {
            to: "status@broadcast".into(),
            message_id: None,
            dry_run: true,
            kind: "status-text".into(),
            message_secret_hex: None,
        });
    }
    let background = parse_argb(background)?;
    let (client, handle) = connect_for_send(config).await?;
    let sent = client
        .status()
        .send_text(
            text,
            background,
            font,
            recipients,
            StatusSendOptions::default(),
        )
        .await?;
    finish_live_command(client, handle).await;
    Ok(SendSummary {
        to: "status@broadcast".into(),
        message_id: Some(sent),
        dry_run: false,
        kind: "status-text".into(),
        message_secret_hex: None,
    })
}

pub async fn status_image(
    config: AppConfig,
    file: &Path,
    thumbnail: Option<&Path>,
    caption: Option<&str>,
    recipients: &[String],
    dry_run: bool,
) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let recipients = parse_jids(recipients)?;
    if dry_run {
        return Ok(SendSummary {
            to: "status@broadcast".into(),
            message_id: None,
            dry_run: true,
            kind: "status-image".into(),
            message_secret_hex: None,
        });
    }
    let data = tokio::fs::read(file)
        .await
        .with_context(|| format!("read {}", file.display()))?;
    let thumb = read_optional_thumbnail(thumbnail).await?;
    let (client, handle) = connect_for_send(config).await?;
    let upload = client.upload(data, MediaType::Image).await?;
    let sent = client
        .status()
        .send_image(
            &upload,
            thumb,
            caption,
            recipients,
            StatusSendOptions::default(),
        )
        .await?;
    finish_live_command(client, handle).await;
    Ok(SendSummary {
        to: "status@broadcast".into(),
        message_id: Some(sent),
        dry_run: false,
        kind: "status-image".into(),
        message_secret_hex: None,
    })
}

pub async fn status_video(
    config: AppConfig,
    file: &Path,
    thumbnail: Option<&Path>,
    duration: u32,
    caption: Option<&str>,
    recipients: &[String],
    dry_run: bool,
) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let recipients = parse_jids(recipients)?;
    if dry_run {
        return Ok(SendSummary {
            to: "status@broadcast".into(),
            message_id: None,
            dry_run: true,
            kind: "status-video".into(),
            message_secret_hex: None,
        });
    }
    let data = tokio::fs::read(file)
        .await
        .with_context(|| format!("read {}", file.display()))?;
    let thumb = read_optional_thumbnail(thumbnail).await?;
    let (client, handle) = connect_for_send(config).await?;
    let upload = client.upload(data, MediaType::Video).await?;
    let sent = client
        .status()
        .send_video(
            &upload,
            thumb,
            duration,
            caption,
            recipients,
            StatusSendOptions::default(),
        )
        .await?;
    finish_live_command(client, handle).await;
    Ok(SendSummary {
        to: "status@broadcast".into(),
        message_id: Some(sent),
        dry_run: false,
        kind: "status-video".into(),
        message_secret_hex: None,
    })
}

pub async fn status_revoke(
    config: AppConfig,
    id: &str,
    recipients: &[String],
    dry_run: bool,
) -> Result<SendSummary> {
    ensure_can_write(&config)?;
    let recipients = parse_jids(recipients)?;
    if dry_run {
        return Ok(SendSummary {
            to: "status@broadcast".into(),
            message_id: Some(id.to_string()),
            dry_run: true,
            kind: "status-revoke".into(),
            message_secret_hex: None,
        });
    }
    let (client, handle) = connect_for_send(config).await?;
    let sent = client
        .status()
        .revoke(id.to_string(), recipients, StatusSendOptions::default())
        .await?;
    finish_live_command(client, handle).await;
    Ok(SendSummary {
        to: "status@broadcast".into(),
        message_id: Some(sent),
        dry_run: false,
        kind: "status-revoke".into(),
        message_secret_hex: None,
    })
}

async fn connect_for_send(
    config: AppConfig,
) -> Result<(Arc<Client>, whatsapp_rust::bot::BotHandle)> {
    connect_for_live_command(config, None).await
}

async fn connect_for_live_command(
    config: AppConfig,
    writer: Option<std::sync::mpsc::Sender<IndexUpdate>>,
) -> Result<(Arc<Client>, whatsapp_rust::bot::BotHandle)> {
    config.ensure_dirs()?;
    let (control_tx, mut control_rx) = mpsc::unbounded_channel();
    let (mut bot, client) =
        build_bot(&config, RunMode::Send, None, None, writer, control_tx).await?;
    let handle = bot.run().await?;
    let timeout = Duration::from_secs(config.timeout_secs);
    let started = Instant::now();
    let mut connected_event = false;

    loop {
        if connected_event && client.is_connected() && client.is_logged_in() {
            break;
        }

        if started.elapsed() >= timeout {
            client.disconnect().await;
            handle.abort();
            return Err(anyhow!(
                "timed out connecting to WhatsApp after {} seconds",
                timeout.as_secs()
            ));
        }

        match tokio::time::timeout(Duration::from_millis(250), control_rx.recv()).await {
            Ok(Some(ControlEvent::Connected)) => {
                connected_event = true;
            }
            Ok(Some(ControlEvent::NeedsAuth)) => {
                client.disconnect().await;
                handle.abort();
                return Err(anyhow!("not authenticated; run `whatshell auth` first"));
            }
            Ok(Some(ControlEvent::LoggedOut)) => {
                client.disconnect().await;
                handle.abort();
                return Err(anyhow!("WhatsApp session logged out; run `whatshell auth`"));
            }
            Ok(Some(_)) | Ok(None) | Err(_) => {}
        }
    }

    Ok((client, handle))
}

async fn finish_live_command(client: Arc<Client>, handle: whatsapp_rust::bot::BotHandle) {
    let _ = tokio::time::timeout(Duration::from_secs(3), client.disconnect()).await;
    let mut handle = handle;
    if tokio::time::timeout(Duration::from_secs(5), &mut handle)
        .await
        .is_err()
    {
        handle.abort();
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }
    drop(client);
}

fn normalize_phones(phones: Vec<String>) -> Result<Vec<String>> {
    phones
        .into_iter()
        .map(|phone| normalize_phone(&phone))
        .collect()
}

fn index_outgoing(
    config: &AppConfig,
    to: &Jid,
    id: &str,
    message: &whatsapp_proto::Message,
) -> Result<()> {
    let mut store = Store::open(&config.index_db())?;
    store.upsert_message(&extract::from_outgoing_message(
        &to.to_string(),
        id,
        message,
    ))?;
    Ok(())
}

fn index_outgoing_preview(
    config: &AppConfig,
    to: &Jid,
    id: &str,
    media_type: &str,
    display_text: &str,
) -> Result<()> {
    let mut store = Store::open(&config.index_db())?;
    store.upsert_message(&MessageInsert {
        chat_jid: to.to_string(),
        chat_name: None,
        msg_id: id.to_string(),
        sender_jid: Some("me".into()),
        sender_name: Some("me".into()),
        ts: chrono::Utc::now().timestamp(),
        from_me: true,
        text: None,
        display_text: Some(display_text.to_string()),
        media_type: Some(media_type.to_string()),
        media_caption: Some(display_text.to_string()),
        filename: None,
        mime_type: None,
        direct_path: None,
        media_key: None,
        file_sha256: None,
        file_enc_sha256: None,
        file_length: None,
        raw_json: None,
    })?;
    Ok(())
}

fn parse_jids(values: &[String]) -> Result<Vec<Jid>> {
    values
        .iter()
        .map(|value| parse_chat_or_phone(value))
        .collect()
}

fn parse_group_participants(values: &[String]) -> Result<Vec<GroupParticipantOptions>> {
    parse_jids(values).map(|jids| {
        jids.into_iter()
            .map(GroupParticipantOptions::from_phone)
            .collect()
    })
}

fn participant_from_row(row: &MessageRecord) -> Result<Option<Jid>> {
    if row.from_me {
        return Ok(None);
    }
    row.sender_jid
        .as_deref()
        .map(parse_chat_or_phone)
        .transpose()
}

fn validate_poll(options: &[String], selectable_count: u32) -> Result<()> {
    if options.len() < 2 {
        return Err(anyhow!("polls require at least two options"));
    }
    if options.len() > 12 {
        return Err(anyhow!("polls support at most 12 options"));
    }
    if selectable_count == 0 || selectable_count as usize > options.len() {
        return Err(anyhow!(
            "selectable_count must be between 1 and {}",
            options.len()
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for option in options {
        if !seen.insert(option) {
            return Err(anyhow!("duplicate poll option: {option}"));
        }
    }
    Ok(())
}

fn media_type_from_store(media_type: &str) -> Result<MediaType> {
    match media_type {
        "image" => Ok(MediaType::Image),
        "video" => Ok(MediaType::Video),
        "audio" => Ok(MediaType::Audio),
        "sticker" => Ok(MediaType::Sticker),
        "document" => Ok(MediaType::Document),
        other => Err(anyhow!("unsupported downloadable media type {other}")),
    }
}

fn resolve_media_output(
    config: &AppConfig,
    media: &MediaRecord,
    output: Option<PathBuf>,
) -> Result<PathBuf> {
    let filename = media.filename.clone().unwrap_or_else(|| {
        let ext = media
            .mime_type
            .as_deref()
            .and_then(mime_guess::get_mime_extensions_str)
            .and_then(|extensions| extensions.first().copied())
            .unwrap_or("bin");
        format!("{}.{}", sanitize_filename(&media.msg_id), ext)
    });
    let path = match output {
        Some(path) if path.is_dir() => path.join(filename),
        Some(path) => path,
        None => config
            .store_dir
            .join("media")
            .join(sanitize_filename(&media.chat_jid))
            .join(filename),
    };
    Ok(path)
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn participant_change_summary(
    row: whatsapp_rust::ParticipantChangeResponse,
) -> ParticipantChangeSummary {
    ParticipantChangeSummary {
        jid: row.jid.to_string(),
        status: row.status,
        error: row.error,
    }
}

fn blocklist_entry_summary(row: BlocklistEntry) -> BlocklistEntrySummary {
    BlocklistEntrySummary {
        jid: row.jid.to_string(),
        timestamp: row.timestamp,
    }
}

fn parse_argb(value: &str) -> Result<u32> {
    let trimmed = value.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u32::from_str_radix(hex, 16).with_context(|| format!("parse ARGB color {value}"))
}

async fn read_optional_thumbnail(path: Option<&Path>) -> Result<Vec<u8>> {
    match path {
        Some(path) => tokio::fs::read(path)
            .await
            .with_context(|| format!("read thumbnail {}", path.display())),
        None => Ok(Vec::new()),
    }
}

fn group_summary(group: whatsapp_rust::GroupMetadata) -> GroupSummary {
    GroupSummary {
        jid: group.id.to_string(),
        subject: group.subject,
        description: group.description,
        size: group.size,
        participant_count: group.participants.len(),
        is_locked: group.is_locked,
        is_announcement: group.is_announcement,
        membership_approval: group.membership_approval,
        participants: group
            .participants
            .into_iter()
            .map(|participant| GroupParticipantSummary {
                jid: participant.jid.to_string(),
                phone_number: participant.phone_number.map(|jid| jid.to_string()),
                is_admin: participant.is_admin,
            })
            .collect(),
    }
}

async fn build_bot(
    config: &AppConfig,
    mode: RunMode,
    phone: Option<String>,
    code: Option<String>,
    writer: Option<std::sync::mpsc::Sender<IndexUpdate>>,
    control_tx: mpsc::UnboundedSender<ControlEvent>,
) -> Result<(Bot, Arc<Client>)> {
    let backend = Arc::new(
        SqliteStore::new(config.session_db().to_string_lossy().as_ref())
            .await
            .context("open WhatsApp session store")?,
    );

    let writer_for_handler = writer.clone();
    let mut builder = Bot::builder()
        .with_backend(backend)
        .with_transport_factory(TokioWebSocketTransportFactory::new())
        .with_http_client(UreqHttpClient::new())
        .with_runtime(TokioRuntime)
        .with_push_name("whatshell")
        .on_event(move |event, client| {
            let writer = writer_for_handler.clone();
            let control_tx = control_tx.clone();
            async move {
                handle_event(mode, event, client, writer, control_tx).await;
            }
        });

    if mode == RunMode::Send {
        builder = builder.skip_history_sync();
    }

    if let Some(phone_number) = phone {
        builder = builder.with_pair_code(PairCodeOptions {
            phone_number,
            custom_code: code,
            ..Default::default()
        });
    }

    let bot = builder.build().await?;
    let client = bot.client();
    Ok((bot, client))
}

async fn handle_event(
    mode: RunMode,
    event: Event,
    client: Arc<Client>,
    writer: Option<std::sync::mpsc::Sender<IndexUpdate>>,
    control_tx: mpsc::UnboundedSender<ControlEvent>,
) {
    match event {
        Event::PairingQrCode { code, timeout } => {
            if mode == RunMode::Auth {
                eprintln!(
                    "Scan this QR in WhatsApp > Linked Devices. Valid for {} seconds.",
                    timeout.as_secs()
                );
                eprintln!("{}", render_qr(&code).unwrap_or(code));
            } else {
                let _ = control_tx.send(ControlEvent::NeedsAuth);
                client.disconnect().await;
            }
        }
        Event::PairingCode { code, timeout } => {
            if mode == RunMode::Auth {
                eprintln!(
                    "Pair code valid for {} seconds: {}",
                    timeout.as_secs(),
                    code
                );
                eprintln!(
                    "Open WhatsApp > Linked Devices > Link a Device > Link with phone number instead."
                );
            }
        }
        Event::PairSuccess(_) => {
            let _ = control_tx.send(ControlEvent::PairSuccess);
        }
        Event::Connected(_) => {
            let _ = control_tx.send(ControlEvent::Connected);
        }
        Event::LoggedOut(_) => {
            let _ = control_tx.send(ControlEvent::LoggedOut);
        }
        Event::Message(message, info) => {
            if let Some(writer) = writer
                && writer
                    .send(IndexUpdate::Message(extract::from_live_message(
                        &message, &info,
                    )))
                    .is_ok()
            {
                let _ = control_tx.send(ControlEvent::Stored);
            }
        }
        Event::HistorySync(sync) => {
            if let Some(writer) = writer {
                for conversation in &sync.conversations {
                    for item in &conversation.messages {
                        if let Some(web_message) = &item.message
                            && let Some(record) =
                                extract::from_history_message(conversation, web_message)
                            && writer.send(IndexUpdate::Message(record)).is_ok()
                        {
                            let _ = control_tx.send(ControlEvent::Stored);
                        }
                    }
                }
                for pushname in &sync.pushnames {
                    if let Some(contact) = contact_from_pushname(pushname)
                        && writer.send(IndexUpdate::Contact(contact)).is_ok()
                    {
                        let _ = control_tx.send(ControlEvent::Stored);
                    }
                }
            }
        }
        Event::ContactUpdate(update) => {
            if let Some(writer) = writer
                && writer
                    .send(IndexUpdate::Contact(contact_from_sync_update(&update)))
                    .is_ok()
            {
                let _ = control_tx.send(ControlEvent::Stored);
            }
        }
        _ => {}
    }
}

fn contact_from_sync_update(update: &whatsapp_rust::types::events::ContactUpdate) -> ContactUpsert {
    let action = &update.action;
    let jid = update.jid.to_string();
    let lid = action.lid_jid.as_deref().and_then(non_empty);
    let phone_jid = action
        .pn_jid
        .as_deref()
        .and_then(non_empty)
        .unwrap_or(jid.as_str());
    let phone_number = phone_from_jid(phone_jid);
    let full_name = action.full_name.as_deref().and_then(non_empty);
    let first_name = action.first_name.as_deref().and_then(non_empty);
    let username = action.username.as_deref().and_then(non_empty);

    ContactUpsert {
        jid,
        lid: lid.map(ToOwned::to_owned),
        phone_number,
        name: full_name.or(first_name).or(username).map(ToOwned::to_owned),
        full_name: full_name.map(ToOwned::to_owned),
        first_name: first_name.map(ToOwned::to_owned),
        username: username.map(ToOwned::to_owned),
        push_name: None,
        source: "whatsapp-app-state".into(),
        from_full_sync: update.from_full_sync,
        updated_at: Some(update.timestamp.timestamp()),
        last_seen_ts: None,
    }
}

fn contact_from_pushname(pushname: &whatsapp_proto::Pushname) -> Option<ContactUpsert> {
    let jid = pushname.id.as_deref().and_then(non_empty)?;
    let name = pushname.pushname.as_deref().and_then(non_empty)?;
    Some(ContactUpsert {
        jid: jid.to_string(),
        lid: None,
        phone_number: phone_from_jid(jid),
        name: Some(name.to_string()),
        full_name: None,
        first_name: None,
        username: None,
        push_name: Some(name.to_string()),
        source: "whatsapp-history-pushname".into(),
        from_full_sync: false,
        updated_at: None,
        last_seen_ts: None,
    })
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn phone_from_jid(jid: &str) -> Option<String> {
    let user = jid.split('@').next().unwrap_or(jid);
    let digits: String = user.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

fn build_text_message(
    to: &Jid,
    text: &str,
    reply_to: Option<&str>,
    reply_sender: Option<&Jid>,
) -> whatsapp_proto::Message {
    if let Some(reply_to) = reply_to {
        whatsapp_proto::Message {
            extended_text_message: Some(Box::new(whatsapp_proto::message::ExtendedTextMessage {
                text: Some(text.to_string()),
                context_info: Some(Box::new(whatsapp_proto::ContextInfo {
                    stanza_id: Some(reply_to.to_string()),
                    remote_jid: Some(to.to_string()),
                    participant: reply_sender.map(ToString::to_string),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        }
    } else {
        whatsapp_proto::Message {
            conversation: Some(text.to_string()),
            ..Default::default()
        }
    }
}

fn build_media_message(
    kind: MediaType,
    mime: &str,
    upload: &whatsapp_rust::upload::UploadResponse,
    caption: Option<String>,
    filename: Option<String>,
) -> whatsapp_proto::Message {
    match kind {
        MediaType::Image => whatsapp_proto::Message {
            image_message: Some(Box::new(whatsapp_proto::message::ImageMessage {
                url: Some(upload.url.clone()),
                mimetype: Some(mime.to_string()),
                caption,
                file_sha256: Some(upload.file_sha256.clone()),
                file_length: Some(upload.file_length),
                media_key: Some(upload.media_key.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                direct_path: Some(upload.direct_path.clone()),
                ..Default::default()
            })),
            ..Default::default()
        },
        MediaType::Video => whatsapp_proto::Message {
            video_message: Some(Box::new(whatsapp_proto::message::VideoMessage {
                url: Some(upload.url.clone()),
                mimetype: Some(mime.to_string()),
                caption,
                file_sha256: Some(upload.file_sha256.clone()),
                file_length: Some(upload.file_length),
                media_key: Some(upload.media_key.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                direct_path: Some(upload.direct_path.clone()),
                ..Default::default()
            })),
            ..Default::default()
        },
        MediaType::Audio => whatsapp_proto::Message {
            audio_message: Some(Box::new(whatsapp_proto::message::AudioMessage {
                url: Some(upload.url.clone()),
                mimetype: Some(mime.to_string()),
                file_sha256: Some(upload.file_sha256.clone()),
                file_length: Some(upload.file_length),
                media_key: Some(upload.media_key.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                direct_path: Some(upload.direct_path.clone()),
                ..Default::default()
            })),
            ..Default::default()
        },
        MediaType::Sticker => whatsapp_proto::Message {
            sticker_message: Some(Box::new(whatsapp_proto::message::StickerMessage {
                url: Some(upload.url.clone()),
                mimetype: Some(mime.to_string()),
                file_sha256: Some(upload.file_sha256.clone()),
                file_length: Some(upload.file_length),
                media_key: Some(upload.media_key.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                direct_path: Some(upload.direct_path.clone()),
                ..Default::default()
            })),
            ..Default::default()
        },
        _ => whatsapp_proto::Message {
            document_message: Some(Box::new(whatsapp_proto::message::DocumentMessage {
                url: Some(upload.url.clone()),
                mimetype: Some(mime.to_string()),
                title: caption.clone(),
                caption,
                file_name: filename,
                file_sha256: Some(upload.file_sha256.clone()),
                file_length: Some(upload.file_length),
                media_key: Some(upload.media_key.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                direct_path: Some(upload.direct_path.clone()),
                ..Default::default()
            })),
            ..Default::default()
        },
    }
}

fn media_type_from_mime(mime: &str) -> MediaType {
    if mime.starts_with("image/webp") {
        MediaType::Sticker
    } else if mime.starts_with("image/") {
        MediaType::Image
    } else if mime.starts_with("video/") {
        MediaType::Video
    } else if mime.starts_with("audio/") {
        MediaType::Audio
    } else {
        MediaType::Document
    }
}

fn kind_name(kind: MediaType) -> &'static str {
    match kind {
        MediaType::Image => "image",
        MediaType::Video => "video",
        MediaType::Audio => "audio",
        MediaType::Sticker => "sticker",
        _ => "document",
    }
}

fn render_qr(code: &str) -> Option<String> {
    let qr = QrCode::new(code.as_bytes()).ok()?;
    Some(
        qr.render::<unicode::Dense1x2>()
            .quiet_zone(true)
            .module_dimensions(2, 1)
            .build(),
    )
}

fn ensure_can_write(config: &AppConfig) -> Result<()> {
    if config.read_only {
        return Err(anyhow!("read-only mode blocks this command"));
    }
    Ok(())
}

pub fn has_session(path: &Path) -> bool {
    path.exists()
}

fn remove_session_files(config: &AppConfig) -> Result<()> {
    let session_db = config.session_db();
    for path in [
        session_db.clone(),
        session_db.with_extension("db-wal"),
        session_db.with_extension("db-shm"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err).with_context(|| format!("remove {}", path.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::proto_helpers::MessageExt;

    #[test]
    fn dry_text_message_uses_conversation_without_reply() {
        let to = parse_chat_or_phone("15551234567").unwrap();
        let msg = build_text_message(&to, "hello", None, None);
        assert_eq!(msg.text_content(), Some("hello"));
    }

    #[test]
    fn dry_text_message_uses_extended_text_for_reply() {
        let to = parse_chat_or_phone("15551234567").unwrap();
        let sender = parse_chat_or_phone("15557654321").unwrap();
        let msg = build_text_message(&to, "hello", Some("abc"), Some(&sender));
        let ext = msg.extended_text_message.unwrap();
        assert_eq!(ext.text.as_deref(), Some("hello"));
        let context = ext.context_info.unwrap();
        assert_eq!(context.stanza_id.as_deref(), Some("abc"));
        assert_eq!(
            context.participant.as_deref(),
            Some("15557654321@s.whatsapp.net")
        );
    }

    #[test]
    fn detects_media_type_from_mime() {
        assert_eq!(media_type_from_mime("image/png"), MediaType::Image);
        assert_eq!(media_type_from_mime("video/mp4"), MediaType::Video);
        assert_eq!(media_type_from_mime("audio/mpeg"), MediaType::Audio);
        assert_eq!(media_type_from_mime("application/pdf"), MediaType::Document);
        assert_eq!(media_type_from_mime("image/webp"), MediaType::Sticker);
    }

    #[test]
    fn blocklist_mapping_prefers_lid_and_phone_jid() {
        let mapping = blocklist_mapping_from_contact(ContactRecord {
            jid: "917386307008@s.whatsapp.net".into(),
            lid: Some("207030509957309@lid".into()),
            phone_number: Some("917386307008".into()),
            name: Some("Devansh Riverline 2".into()),
            full_name: None,
            first_name: None,
            username: None,
            push_name: None,
            source: "whatsapp-app-state".into(),
            from_full_sync: true,
            updated_at: None,
            last_seen_ts: None,
        })
        .unwrap();

        assert_eq!(mapping.lid.unwrap().to_string(), "207030509957309@lid");
        assert_eq!(
            mapping.pn.unwrap().to_string(),
            "917386307008@s.whatsapp.net"
        );
    }
}
