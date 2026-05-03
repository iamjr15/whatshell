use anyhow::Result;
use serde::Serialize;
use whatsapp_rust::proto_helpers::MessageExt;
use whatsapp_rust::types::message::MessageInfo;
use whatsapp_rust::waproto::whatsapp as whatsapp_proto;

use crate::store::MessageInsert;

pub fn from_live_message(message: &whatsapp_proto::Message, info: &MessageInfo) -> MessageInsert {
    let media = media_fields(message);
    let text = message.text_content().map(ToOwned::to_owned);
    let display_text = display_text(text.as_deref(), media.caption.as_deref(), media.kind);

    MessageInsert {
        chat_jid: info.source.chat.to_string(),
        chat_name: None,
        msg_id: info.id.clone(),
        sender_jid: Some(info.source.sender.to_string()),
        sender_name: non_empty(&info.push_name),
        ts: info.timestamp.timestamp(),
        from_me: info.source.is_from_me,
        text,
        display_text,
        media_type: media.kind.map(str::to_string),
        media_caption: media.caption,
        filename: media.filename,
        mime_type: media.mime_type,
        direct_path: media.direct_path,
        media_key: media.media_key,
        file_sha256: media.file_sha256,
        file_enc_sha256: media.file_enc_sha256,
        file_length: media.file_length.map(|v| v as i64),
        raw_json: serialize_json(message).ok(),
    }
}

pub fn from_history_message(
    conversation: &whatsapp_proto::Conversation,
    message: &whatsapp_proto::WebMessageInfo,
) -> Option<MessageInsert> {
    let payload = message.message.as_ref()?;
    let key = &message.key;
    let chat_jid = key
        .remote_jid
        .clone()
        .or_else(|| Some(conversation.id.clone()))?;
    let msg_id = key.id.clone()?;
    let from_me = key.from_me.unwrap_or(false);
    let sender_jid = if from_me {
        Some("me".to_string())
    } else {
        key.participant
            .clone()
            .or_else(|| message.participant.clone())
            .or_else(|| {
                if chat_jid.ends_with("@g.us") {
                    None
                } else {
                    Some(chat_jid.clone())
                }
            })
    };

    let media = media_fields(payload);
    let text = payload.text_content().map(ToOwned::to_owned);
    let display_text = display_text(text.as_deref(), media.caption.as_deref(), media.kind);

    Some(MessageInsert {
        chat_jid,
        chat_name: conversation.name.clone(),
        msg_id,
        sender_jid,
        sender_name: message.push_name.clone(),
        ts: message.message_timestamp.unwrap_or(0) as i64,
        from_me,
        text,
        display_text,
        media_type: media.kind.map(str::to_string),
        media_caption: media.caption,
        filename: media.filename,
        mime_type: media.mime_type,
        direct_path: media.direct_path,
        media_key: media.media_key,
        file_sha256: media.file_sha256,
        file_enc_sha256: media.file_enc_sha256,
        file_length: media.file_length.map(|v| v as i64),
        raw_json: serialize_json(message).ok(),
    })
}

pub fn from_outgoing_message(
    chat_jid: &str,
    msg_id: &str,
    message: &whatsapp_proto::Message,
) -> MessageInsert {
    let media = media_fields(message);
    let text = message.text_content().map(ToOwned::to_owned);
    let display_text = display_text(text.as_deref(), media.caption.as_deref(), media.kind);

    MessageInsert {
        chat_jid: chat_jid.to_string(),
        chat_name: None,
        msg_id: msg_id.to_string(),
        sender_jid: Some("me".to_string()),
        sender_name: Some("me".to_string()),
        ts: chrono::Utc::now().timestamp(),
        from_me: true,
        text,
        display_text,
        media_type: media.kind.map(str::to_string),
        media_caption: media.caption,
        filename: media.filename,
        mime_type: media.mime_type,
        direct_path: media.direct_path,
        media_key: media.media_key,
        file_sha256: media.file_sha256,
        file_enc_sha256: media.file_enc_sha256,
        file_length: media.file_length.map(|v| v as i64),
        raw_json: serialize_json(message).ok(),
    }
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn display_text(text: Option<&str>, caption: Option<&str>, kind: Option<&str>) -> Option<String> {
    if let Some(text) = text
        && !text.is_empty()
    {
        return Some(text.to_string());
    }
    if let Some(caption) = caption
        && !caption.is_empty()
    {
        return Some(caption.to_string());
    }
    kind.map(|kind| format!("[{kind}]"))
}

#[derive(Debug, Default)]
struct MediaFields {
    kind: Option<&'static str>,
    caption: Option<String>,
    filename: Option<String>,
    mime_type: Option<String>,
    direct_path: Option<String>,
    media_key: Option<Vec<u8>>,
    file_sha256: Option<Vec<u8>>,
    file_enc_sha256: Option<Vec<u8>>,
    file_length: Option<u64>,
}

fn media_fields(message: &whatsapp_proto::Message) -> MediaFields {
    let base = message.get_base_message();
    if let Some(image) = &base.image_message {
        return MediaFields {
            kind: Some("image"),
            caption: image.caption.clone(),
            mime_type: image.mimetype.clone(),
            direct_path: image.direct_path.clone(),
            media_key: image.media_key.clone(),
            file_sha256: image.file_sha256.clone(),
            file_enc_sha256: image.file_enc_sha256.clone(),
            file_length: image.file_length,
            ..Default::default()
        };
    }
    if let Some(video) = &base.video_message {
        return MediaFields {
            kind: Some("video"),
            caption: video.caption.clone(),
            mime_type: video.mimetype.clone(),
            direct_path: video.direct_path.clone(),
            media_key: video.media_key.clone(),
            file_sha256: video.file_sha256.clone(),
            file_enc_sha256: video.file_enc_sha256.clone(),
            file_length: video.file_length,
            ..Default::default()
        };
    }
    if let Some(audio) = &base.audio_message {
        return MediaFields {
            kind: Some("audio"),
            mime_type: audio.mimetype.clone(),
            direct_path: audio.direct_path.clone(),
            media_key: audio.media_key.clone(),
            file_sha256: audio.file_sha256.clone(),
            file_enc_sha256: audio.file_enc_sha256.clone(),
            file_length: audio.file_length,
            ..Default::default()
        };
    }
    if let Some(document) = &base.document_message {
        return MediaFields {
            kind: Some("document"),
            caption: document.caption.clone().or_else(|| document.title.clone()),
            filename: document.file_name.clone(),
            mime_type: document.mimetype.clone(),
            direct_path: document.direct_path.clone(),
            media_key: document.media_key.clone(),
            file_sha256: document.file_sha256.clone(),
            file_enc_sha256: document.file_enc_sha256.clone(),
            file_length: document.file_length,
        };
    }
    if let Some(sticker) = &base.sticker_message {
        return MediaFields {
            kind: Some("sticker"),
            mime_type: sticker.mimetype.clone(),
            direct_path: sticker.direct_path.clone(),
            media_key: sticker.media_key.clone(),
            file_sha256: sticker.file_sha256.clone(),
            file_enc_sha256: sticker.file_enc_sha256.clone(),
            file_length: sticker.file_length,
            ..Default::default()
        };
    }
    if let Some(reaction) = &base.reaction_message {
        return MediaFields {
            kind: Some("reaction"),
            caption: reaction.text.clone().filter(|text| !text.is_empty()),
            ..Default::default()
        };
    }
    if let Some(poll) = base
        .poll_creation_message
        .as_ref()
        .or(base.poll_creation_message_v2.as_ref())
        .or(base.poll_creation_message_v3.as_ref())
    {
        return MediaFields {
            kind: Some("poll"),
            caption: poll.name.clone(),
            ..Default::default()
        };
    }
    if let Some(update) = &base.poll_update_message {
        return MediaFields {
            kind: Some("poll"),
            caption: update
                .poll_creation_message_key
                .as_ref()
                .and_then(|key| key.id.clone())
                .map(|id| format!("poll vote for {id}")),
            ..Default::default()
        };
    }
    if let Some(location) = &base.location_message {
        let caption = location.name.clone().or_else(|| {
            match (location.degrees_latitude, location.degrees_longitude) {
                (Some(lat), Some(lng)) => Some(format!("{lat},{lng}")),
                _ => None,
            }
        });
        return MediaFields {
            kind: Some("location"),
            caption,
            ..Default::default()
        };
    }
    if let Some(contact) = &base.contact_message {
        return MediaFields {
            kind: Some("contact"),
            caption: contact.display_name.clone(),
            ..Default::default()
        };
    }
    if let Some(contacts) = &base.contacts_array_message {
        return MediaFields {
            kind: Some("contact"),
            caption: contacts.display_name.clone().or_else(|| {
                Some(format!("{} contacts", contacts.contacts.len()))
                    .filter(|_| !contacts.contacts.is_empty())
            }),
            ..Default::default()
        };
    }
    if let Some(protocol) = &base.protocol_message {
        return MediaFields {
            kind: Some("protocol"),
            caption: protocol_message_name(protocol.r#type),
            ..Default::default()
        };
    }

    MediaFields::default()
}

fn protocol_message_name(value: Option<i32>) -> Option<String> {
    value
        .and_then(|kind| whatsapp_proto::message::protocol_message::Type::try_from(kind).ok())
        .map(|kind| format!("{kind:?}"))
        .or_else(|| value.map(|kind| format!("type {kind}")))
}
