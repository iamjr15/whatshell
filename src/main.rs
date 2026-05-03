use std::process::ExitCode;

use anyhow::{Result, anyhow};
use clap::Parser;
use serde::Serialize;

mod cli;
mod config;
mod extract;
mod jid;
mod lock;
mod output;
mod store;
mod whatshell;

use cli::{
    AuthSubcommand, BlockingSubcommand, ChatsSubcommand, Cli, Command, ContactsSubcommand,
    ExportFormat, ExportSubcommand, GroupsSubcommand, MediaSubcommand, MessagesSubcommand,
    PresenceSubcommand, ProfileSubcommand, SendSubcommand, StatusSubcommand,
};
use config::AppConfig;
use lock::StoreLock;
use store::{MessageFilter, Store, StoreStats};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let json = cli.json;
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if json {
                let _ = output::json(&ErrorResponse {
                    success: false,
                    error: err.to_string(),
                });
            } else {
                eprintln!("error: {err:#}");
            }
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let config = AppConfig::new(cli.store, cli.json, cli.timeout, cli.read_only)?;

    match cli.command {
        Command::Auth(args) => match args.command {
            Some(AuthSubcommand::Status) => auth_status(&config),
            Some(AuthSubcommand::Logout) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                whatshell::logout(config).await?;
                if cli.json {
                    output::json_response(StatusMessage::new("logged out"))
                } else {
                    output::print_status("logged out");
                    Ok(())
                }
            }
            None => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                whatshell::auth(config, args).await
            }
        },
        Command::Sync(args) => {
            let _lock = StoreLock::acquire(&config.lock_path())?;
            whatshell::sync(config, args).await
        }
        Command::Listen(args) => {
            let _lock = StoreLock::acquire(&config.lock_path())?;
            whatshell::listen(config, args.stream_jsonl).await
        }
        Command::Send(args) => {
            let _lock = StoreLock::acquire(&config.lock_path())?;
            match args.command {
                SendSubcommand::Text(args) => {
                    print_send_summary(&config, whatshell::send_text(config.clone(), args).await?)
                }
                SendSubcommand::File(args) => {
                    print_send_summary(&config, whatshell::send_file(config.clone(), args).await?)
                }
                SendSubcommand::React(args) => {
                    print_send_summary(&config, whatshell::send_react(config.clone(), args).await?)
                }
                SendSubcommand::Poll(args) => {
                    print_send_summary(&config, whatshell::send_poll(config.clone(), args).await?)
                }
                SendSubcommand::Location(args) => print_send_summary(
                    &config,
                    whatshell::send_location(config.clone(), args).await?,
                ),
                SendSubcommand::Contact(args) => print_send_summary(
                    &config,
                    whatshell::send_contact(config.clone(), args).await?,
                ),
            }
        }
        Command::Messages(args) => handle_messages(config, args.command).await,
        Command::Chats(args) => match args.command {
            ChatsSubcommand::List(args) => {
                let store = open_store_for_read(&config)?;
                let rows = store.list_chats(args.query.as_deref(), args.limit)?;
                if config.json {
                    output::json_response(rows)
                } else {
                    output::print_chats(&rows);
                    Ok(())
                }
            }
            ChatsSubcommand::Archive(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_archive(config.clone(), &args.chat, true).await?,
                )
            }
            ChatsSubcommand::Unarchive(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_archive(config.clone(), &args.chat, false).await?,
                )
            }
            ChatsSubcommand::Pin(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_pin(config.clone(), &args.chat, true).await?,
                )
            }
            ChatsSubcommand::Unpin(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_pin(config.clone(), &args.chat, false).await?,
                )
            }
            ChatsSubcommand::Mute(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_mute(config.clone(), &args.chat, true).await?,
                )
            }
            ChatsSubcommand::Unmute(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_mute(config.clone(), &args.chat, false).await?,
                )
            }
            ChatsSubcommand::MarkRead(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_mark_read(config.clone(), &args.chat, !args.unread).await?,
                )
            }
            ChatsSubcommand::Delete(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::chat_delete(config.clone(), &args.chat, args.delete_media).await?,
                )
            }
        },
        Command::Contacts(args) => match args.command {
            ContactsSubcommand::Sync(_) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                output::json_response(whatshell::contacts_sync(config.clone()).await?)
            }
            ContactsSubcommand::List(args) => {
                sync_contacts_unless_offline(&config, args.offline).await?;
                let store = open_store_for_read(&config)?;
                let rows = store.list_contact_book(args.query.as_deref(), args.limit)?;
                if config.json {
                    output::json_response(rows)
                } else {
                    output::print_contacts(&rows);
                    Ok(())
                }
            }
            ContactsSubcommand::Search(args) => {
                sync_contacts_unless_offline(&config, args.offline).await?;
                let rows = search_contacts(&config, &args.query, args.limit)?;
                output::json_response(rows)
            }
            ContactsSubcommand::Check(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                let rows = whatshell::contacts_check(config.clone(), args.phones).await?;
                output::json_response(rows)
            }
            ContactsSubcommand::Info(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                let rows = whatshell::contacts_info(config.clone(), args.phones).await?;
                output::json_response(rows)
            }
        },
        Command::Groups(args) => match args.command {
            GroupsSubcommand::List(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                let rows = whatshell::groups_list(config.clone(), args.query).await?;
                output::json_response(rows)
            }
            GroupsSubcommand::Info(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                output::json_response(whatshell::groups_info(config.clone(), &args.jid).await?)
            }
            GroupsSubcommand::Create(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_create(
                        config.clone(),
                        &args.subject,
                        &args.participants,
                        args.admins_only_add,
                        args.approval,
                        args.ephemeral_seconds,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            GroupsSubcommand::SetSubject(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_set_subject(
                        config.clone(),
                        &args.jid,
                        &args.subject,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            GroupsSubcommand::SetDescription(args) => {
                if args.clear && args.description.is_some() {
                    return Err(anyhow!("--clear and --description cannot be used together"));
                }
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_set_description(
                        config.clone(),
                        &args.jid,
                        if args.clear { None } else { args.description },
                        args.dry_run,
                    )
                    .await?,
                )
            }
            GroupsSubcommand::InviteLink(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_invite_link(config.clone(), &args.jid, args.reset).await?,
                )
            }
            GroupsSubcommand::InviteInfo(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                output::json_response(
                    whatshell::groups_invite_info(config.clone(), &args.code).await?,
                )
            }
            GroupsSubcommand::Leave(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_leave(config.clone(), &args.jid).await?,
                )
            }
            GroupsSubcommand::Add(args) => handle_group_participants(config, args, "add").await,
            GroupsSubcommand::Remove(args) => {
                handle_group_participants(config, args, "remove").await
            }
            GroupsSubcommand::Promote(args) => {
                handle_group_participants(config, args, "promote").await
            }
            GroupsSubcommand::Demote(args) => {
                handle_group_participants(config, args, "demote").await
            }
            GroupsSubcommand::Approve(args) => {
                handle_group_participants(config, args, "approve").await
            }
            GroupsSubcommand::Reject(args) => {
                handle_group_participants(config, args, "reject").await
            }
            GroupsSubcommand::Lock(args) => handle_group_setting(config, args, "lock").await,
            GroupsSubcommand::Unlock(args) => handle_group_setting(config, args, "unlock").await,
            GroupsSubcommand::Announce(args) => {
                handle_group_setting(config, args, "announce").await
            }
            GroupsSubcommand::Unannounce(args) => {
                handle_group_setting(config, args, "unannounce").await
            }
            GroupsSubcommand::Ephemeral(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_set_ephemeral(
                        config.clone(),
                        &args.jid,
                        args.seconds,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            GroupsSubcommand::Approval(args) => {
                if args.on == args.off {
                    return Err(anyhow!("pass exactly one of --on or --off"));
                }
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_set_approval(
                        config.clone(),
                        &args.jid,
                        args.on,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            GroupsSubcommand::MemberAdd(args) => {
                if args.admins_only == args.everyone {
                    return Err(anyhow!("pass exactly one of --admins-only or --everyone"));
                }
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::groups_set_member_add(
                        config.clone(),
                        &args.jid,
                        args.admins_only,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            GroupsSubcommand::Requests(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                output::json_response(whatshell::groups_requests(config.clone(), &args.jid).await?)
            }
        },
        Command::Media(args) => match args.command {
            MediaSubcommand::List(args) => {
                let store = open_store_for_read(&config)?;
                let chat = normalize_optional(args.chat.as_deref())?;
                let media_type = args
                    .r#type
                    .as_ref()
                    .and_then(|filter| filter.as_store_value())
                    .filter(|kind| *kind != "text");
                let rows = store.list_media(chat.as_deref(), media_type, args.limit)?;
                print_message_rows(&config, &rows)
            }
            MediaSubcommand::Download(args) => {
                let chat = jid::normalize_chat_string(&args.chat)?;
                let store = open_store_for_read(&config)?;
                let media = store.media_message(&chat, &args.id)?.ok_or_else(|| {
                    anyhow!(
                        "downloadable media not found for chat {chat} and id {}",
                        args.id
                    )
                })?;
                let _lock = StoreLock::acquire(&config.lock_path())?;
                let summary =
                    whatshell::media_download(config.clone(), media, args.output, args.overwrite)
                        .await?;
                if config.json {
                    output::json_response(summary)
                } else {
                    println!("{}", summary.path);
                    Ok(())
                }
            }
        },
        Command::Presence(args) => match args.command {
            PresenceSubcommand::Online => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::presence_set(config.clone(), true).await?,
                )
            }
            PresenceSubcommand::Offline => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::presence_set(config.clone(), false).await?,
                )
            }
            PresenceSubcommand::Typing(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::presence_chatstate(config.clone(), &args.chat, "typing").await?,
                )
            }
            PresenceSubcommand::Recording(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::presence_chatstate(config.clone(), &args.chat, "recording").await?,
                )
            }
            PresenceSubcommand::Paused(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::presence_chatstate(config.clone(), &args.chat, "paused").await?,
                )
            }
        },
        Command::Profile(args) => match args.command {
            ProfileSubcommand::SetName(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::profile_set_name(config.clone(), &args.text, args.dry_run).await?,
                )
            }
            ProfileSubcommand::SetAbout(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::profile_set_about(config.clone(), &args.text, args.dry_run).await?,
                )
            }
            ProfileSubcommand::SetPicture(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::profile_set_picture(config.clone(), &args.file, args.dry_run)
                        .await?,
                )
            }
            ProfileSubcommand::RemovePicture => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::profile_remove_picture(config.clone()).await?,
                )
            }
        },
        Command::Blocking(args) => match args.command {
            BlockingSubcommand::List => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                output::json_response(whatshell::blocking_list(config.clone()).await?)
            }
            BlockingSubcommand::Block(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::blocking_set(config.clone(), &args.contact, true, args.dry_run)
                        .await?,
                )
            }
            BlockingSubcommand::Unblock(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_action_summary(
                    &config,
                    whatshell::blocking_set(config.clone(), &args.contact, false, args.dry_run)
                        .await?,
                )
            }
            BlockingSubcommand::IsBlocked(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                output::json_response(
                    whatshell::blocking_is_blocked(config.clone(), &args.contact).await?,
                )
            }
        },
        Command::Status(args) => match args.command {
            StatusSubcommand::Text(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_send_summary(
                    &config,
                    whatshell::status_text(
                        config.clone(),
                        &args.message,
                        &args.recipients,
                        &args.background,
                        args.font,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            StatusSubcommand::Image(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_send_summary(
                    &config,
                    whatshell::status_image(
                        config.clone(),
                        &args.file,
                        args.thumbnail.as_deref(),
                        args.caption.as_deref(),
                        &args.recipients,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            StatusSubcommand::Video(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_send_summary(
                    &config,
                    whatshell::status_video(
                        config.clone(),
                        &args.file,
                        args.thumbnail.as_deref(),
                        args.duration,
                        args.caption.as_deref(),
                        &args.recipients,
                        args.dry_run,
                    )
                    .await?,
                )
            }
            StatusSubcommand::Revoke(args) => {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                print_send_summary(
                    &config,
                    whatshell::status_revoke(
                        config.clone(),
                        &args.id,
                        &args.recipients,
                        args.dry_run,
                    )
                    .await?,
                )
            }
        },
        Command::Export(args) => match args.command {
            ExportSubcommand::Messages(args) => handle_export_messages(&config, args),
            ExportSubcommand::Analytics(args) => handle_export_analytics(&config, args),
        },
        Command::Doctor(args) => {
            let report = doctor_report(&config)?;
            if config.json {
                output::json_response(&report)?;
            } else {
                print_doctor_report(&report);
            }

            if args.connect {
                let _lock = StoreLock::acquire(&config.lock_path())?;
                whatshell::check_connection(config.clone()).await?;
                if !config.json {
                    output::print_status("connection: ok");
                }
            }
            Ok(())
        }
    }
}

async fn handle_messages(config: AppConfig, command: MessagesSubcommand) -> Result<()> {
    match command {
        MessagesSubcommand::List(args) => {
            let store = open_store_for_read(&config)?;
            if args.from_me && args.from_them {
                return Err(anyhow!("--from-me and --from-them cannot be used together"));
            }
            let rows = store.list_messages(&MessageFilter {
                chat: normalize_optional(args.chat.as_deref())?,
                sender: normalize_optional(args.sender.as_deref())?,
                from_me: args
                    .from_me
                    .then_some(true)
                    .or(args.from_them.then_some(false)),
                media_type: args
                    .r#type
                    .as_ref()
                    .and_then(|filter| filter.as_store_value().map(ToOwned::to_owned)),
                limit: args.limit,
                asc: args.asc,
            })?;
            print_message_rows(&config, &rows)
        }
        MessagesSubcommand::Search(args) => {
            let store = open_store_for_read(&config)?;
            let rows = store.search_messages(
                &args.query,
                &MessageFilter {
                    chat: normalize_optional(args.chat.as_deref())?,
                    sender: normalize_optional(args.sender.as_deref())?,
                    from_me: None,
                    media_type: args
                        .r#type
                        .as_ref()
                        .and_then(|filter| filter.as_store_value().map(ToOwned::to_owned)),
                    limit: args.limit,
                    asc: false,
                },
            )?;
            print_message_rows(&config, &rows)
        }
        MessagesSubcommand::Show(args) => {
            let store = open_store_for_read(&config)?;
            let chat = jid::normalize_chat_string(&args.chat)?;
            let row = store
                .show_message(&chat, &args.id)?
                .ok_or_else(|| anyhow!("message not found for chat {chat} and id {}", args.id))?;
            print_message_rows(&config, &[row])
        }
        MessagesSubcommand::Context(args) => {
            let store = open_store_for_read(&config)?;
            let chat = jid::normalize_chat_string(&args.chat)?;
            let rows = store.message_context(&chat, &args.id, args.before, args.after)?;
            if rows.is_empty() {
                return Err(anyhow!(
                    "message not found for chat {chat} and id {}",
                    args.id
                ));
            }
            print_message_rows(&config, &rows)
        }
        MessagesSubcommand::Reply(args) => {
            let store = open_store_for_read(&config)?;
            let chat = jid::normalize_chat_string(&args.chat)?;
            let row = store
                .show_message(&chat, &args.id)?
                .ok_or_else(|| anyhow!("message not found for chat {chat} and id {}", args.id))?;
            let _lock = StoreLock::acquire(&config.lock_path())?;
            print_send_summary(
                &config,
                whatshell::send_text(
                    config.clone(),
                    cli::SendText {
                        to: chat,
                        message: args.message,
                        reply_to: Some(row.msg_id),
                        reply_sender: (!row.from_me).then_some(row.sender_jid).flatten(),
                        dry_run: args.dry_run,
                    },
                )
                .await?,
            )
        }
        MessagesSubcommand::Edit(args) => {
            let _lock = StoreLock::acquire(&config.lock_path())?;
            print_send_summary(
                &config,
                whatshell::message_edit(
                    config.clone(),
                    &args.chat,
                    &args.id,
                    &args.message,
                    args.dry_run,
                )
                .await?,
            )
        }
        MessagesSubcommand::Revoke(args) => {
            let _lock = StoreLock::acquire(&config.lock_path())?;
            print_action_summary(
                &config,
                whatshell::message_revoke(
                    config.clone(),
                    &args.chat,
                    &args.id,
                    args.admin_sender.as_deref(),
                    args.dry_run,
                )
                .await?,
            )
        }
        MessagesSubcommand::DeleteForMe(args) => {
            let store = open_store_for_read(&config)?;
            let row = load_message_row(&store, &args.chat, &args.id)?;
            let _lock = StoreLock::acquire(&config.lock_path())?;
            print_action_summary(
                &config,
                whatshell::message_delete_for_me(
                    config.clone(),
                    &row,
                    args.delete_media,
                    args.dry_run,
                )
                .await?,
            )
        }
        MessagesSubcommand::Star(args) => {
            let store = open_store_for_read(&config)?;
            let row = load_message_row(&store, &args.chat, &args.id)?;
            let _lock = StoreLock::acquire(&config.lock_path())?;
            print_action_summary(
                &config,
                whatshell::message_star(config.clone(), &row, true, args.dry_run).await?,
            )
        }
        MessagesSubcommand::Unstar(args) => {
            let store = open_store_for_read(&config)?;
            let row = load_message_row(&store, &args.chat, &args.id)?;
            let _lock = StoreLock::acquire(&config.lock_path())?;
            print_action_summary(
                &config,
                whatshell::message_star(config.clone(), &row, false, args.dry_run).await?,
            )
        }
    }
}

async fn handle_group_participants(
    config: AppConfig,
    args: cli::GroupParticipants,
    action: &str,
) -> Result<()> {
    let _lock = StoreLock::acquire(&config.lock_path())?;
    let rows = whatshell::groups_participants(
        config.clone(),
        &args.jid,
        &args.participants,
        action,
        args.dry_run,
    )
    .await?;
    output::json_response(rows)
}

async fn handle_group_setting(
    config: AppConfig,
    args: cli::GroupTarget,
    action: &str,
) -> Result<()> {
    let _lock = StoreLock::acquire(&config.lock_path())?;
    print_action_summary(
        &config,
        whatshell::groups_setting(config.clone(), &args.jid, action, false).await?,
    )
}

fn handle_export_messages(config: &AppConfig, args: cli::ExportMessages) -> Result<()> {
    if args.from_me && args.from_them {
        return Err(anyhow!("--from-me and --from-them cannot be used together"));
    }
    let store = open_store_for_read(config)?;
    let rows = store.list_messages(&MessageFilter {
        chat: normalize_optional(args.chat.as_deref())?,
        sender: normalize_optional(args.sender.as_deref())?,
        from_me: args
            .from_me
            .then_some(true)
            .or(args.from_them.then_some(false)),
        media_type: args
            .r#type
            .as_ref()
            .and_then(|filter| filter.as_store_value().map(ToOwned::to_owned)),
        limit: args.limit,
        asc: true,
    })?;
    let body = match args.format {
        ExportFormat::Json => serde_json::to_string_pretty(&rows)?,
        ExportFormat::Jsonl => rows
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n"),
        ExportFormat::Csv => messages_csv(&rows),
    };
    write_export(args.output.as_deref(), &body)
}

fn handle_export_analytics(config: &AppConfig, args: cli::ExportAnalytics) -> Result<()> {
    let store = open_store_for_read(config)?;
    let chat = normalize_optional(args.chat.as_deref())?;
    let report = store.analytics(chat.as_deref())?;
    let body = serde_json::to_string_pretty(&report)?;
    write_export(args.output.as_deref(), &body)
}

fn write_export(path: Option<&std::path::Path>, body: &str) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
    } else {
        println!("{body}");
    }
    Ok(())
}

fn messages_csv(rows: &[store::MessageRecord]) -> String {
    let mut out = String::from(
        "ts,chat_jid,chat_name,msg_id,sender_jid,sender_name,from_me,type,text,filename,mime_type,local_path\n",
    );
    for row in rows {
        let fields = [
            row.ts.to_string(),
            row.chat_jid.clone(),
            row.chat_name.clone().unwrap_or_default(),
            row.msg_id.clone(),
            row.sender_jid.clone().unwrap_or_default(),
            row.sender_name.clone().unwrap_or_default(),
            row.from_me.to_string(),
            row.media_type.clone().unwrap_or_else(|| "text".into()),
            row.display_text.clone().unwrap_or_default(),
            row.filename.clone().unwrap_or_default(),
            row.mime_type.clone().unwrap_or_default(),
            row.local_path.clone().unwrap_or_default(),
        ];
        out.push_str(
            &fields
                .iter()
                .map(|field| csv_escape(field))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn load_message_row(store: &Store, chat: &str, id: &str) -> Result<store::MessageRecord> {
    let chat = jid::normalize_chat_string(chat)?;
    store
        .show_message(&chat, id)?
        .ok_or_else(|| anyhow!("message not found for chat {chat} and id {id}"))
}

async fn sync_contacts_unless_offline(config: &AppConfig, offline: bool) -> Result<()> {
    if offline {
        return Ok(());
    }
    if config.read_only {
        return Err(anyhow!(
            "contact sync needs write access to the whatshell index; pass --offline to search the existing index"
        ));
    }
    let _lock = StoreLock::acquire(&config.lock_path())?;
    whatshell::contacts_sync(config.clone()).await?;
    Ok(())
}

fn search_contacts(config: &AppConfig, query: &str, limit: usize) -> Result<Vec<ContactSearchRow>> {
    let mut rows = Vec::new();
    let store = open_store_for_read(config)?;
    for row in store.list_contact_book(Some(query), limit)? {
        rows.push(ContactSearchRow {
            source: row.source,
            name: row
                .name
                .or(row.full_name.clone())
                .or(row.first_name.clone())
                .or(row.username.clone())
                .or(row.push_name.clone()),
            full_name: row.full_name,
            first_name: row.first_name,
            push_name: row.push_name,
            jid: row.jid,
            phone: row.phone_number,
            lid: row.lid,
            username: row.username,
            about: None,
        });
    }
    for row in store.list_contacts(Some(query), limit)? {
        rows.push(ContactSearchRow {
            source: "whatshell-index".into(),
            name: row.name,
            full_name: None,
            first_name: None,
            push_name: None,
            jid: row.jid,
            phone: None,
            lid: None,
            username: None,
            about: None,
        });
    }
    dedupe_contacts(rows, limit)
}

fn dedupe_contacts(rows: Vec<ContactSearchRow>, limit: usize) -> Result<Vec<ContactSearchRow>> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let key = if !row.jid.is_empty() {
            row.jid.clone()
        } else {
            row.phone.clone().unwrap_or_default()
        };
        if seen.insert(key) {
            out.push(row);
            if out.len() >= limit.max(1) {
                break;
            }
        }
    }
    Ok(out)
}

fn auth_status(config: &AppConfig) -> Result<()> {
    let index = try_store_stats(config)?;
    let status = AuthStatus {
        authenticated: whatshell::has_session(&config.session_db()),
        session_db: config.session_db().display().to_string(),
        index_db: config.index_db().display().to_string(),
        store_dir: config.store_dir.display().to_string(),
        index,
    };

    if config.json {
        output::json_response(status)
    } else {
        println!("authenticated: {}", status.authenticated);
        println!("store_dir: {}", status.store_dir);
        println!("session_db: {}", status.session_db);
        println!("index_db: {}", status.index_db);
        if let Some(index) = status.index {
            println!("chats: {}", index.chats);
            println!("messages: {}", index.messages);
            println!("fts: {}", index.fts);
        }
        Ok(())
    }
}

fn doctor_report(config: &AppConfig) -> Result<DoctorReport> {
    Ok(DoctorReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        store_dir: config.store_dir.display().to_string(),
        session_db: config.session_db().display().to_string(),
        index_db: config.index_db().display().to_string(),
        session_present: whatshell::has_session(&config.session_db()),
        read_only: config.read_only,
        timeout_secs: config.timeout_secs,
        index: try_store_stats(config)?,
    })
}

fn print_doctor_report(report: &DoctorReport) {
    println!("whatshell {}", report.version);
    println!("store_dir: {}", report.store_dir);
    println!("session_db: {}", report.session_db);
    println!("index_db: {}", report.index_db);
    println!("session_present: {}", report.session_present);
    println!("read_only: {}", report.read_only);
    println!("timeout_secs: {}", report.timeout_secs);
    if let Some(index) = &report.index {
        println!("chats: {}", index.chats);
        println!("messages: {}", index.messages);
        println!("fts: {}", index.fts);
    }
}

fn try_store_stats(config: &AppConfig) -> Result<Option<StoreStats>> {
    let path = config.index_db();
    if !path.exists() {
        return Ok(None);
    }
    let store = Store::open_readonly(&path)?;
    Ok(Some(store.stats(&path)?))
}

fn open_store_for_read(config: &AppConfig) -> Result<Store> {
    if config.read_only {
        return Store::open_readonly(&config.index_db());
    }

    config.ensure_dirs()?;
    Store::open(&config.index_db())
}

fn normalize_optional(value: Option<&str>) -> Result<Option<String>> {
    value.map(jid::normalize_chat_string).transpose()
}

fn print_message_rows(config: &AppConfig, rows: &[store::MessageRecord]) -> Result<()> {
    if config.json {
        output::json_response(rows)
    } else {
        output::print_messages(rows);
        Ok(())
    }
}

fn print_send_summary(config: &AppConfig, summary: whatshell::SendSummary) -> Result<()> {
    if config.json {
        output::json_response(summary)
    } else if summary.dry_run {
        println!("dry-run {} message to {}", summary.kind, summary.to);
        Ok(())
    } else {
        println!("{}", summary.message_id.as_deref().unwrap_or("sent"));
        Ok(())
    }
}

fn print_action_summary(config: &AppConfig, summary: whatshell::ActionSummary) -> Result<()> {
    if config.json {
        output::json_response(summary)
    } else {
        println!("{} {}", summary.action, summary.target);
        Ok(())
    }
}

fn init_tracing(verbose: bool) {
    let filter = if verbose {
        "whatshell=debug,whatsapp_rust=info"
    } else {
        "error"
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

#[derive(Debug, Serialize)]
struct StatusMessage {
    message: String,
}

impl StatusMessage {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct AuthStatus {
    authenticated: bool,
    store_dir: String,
    session_db: String,
    index_db: String,
    index: Option<StoreStats>,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    version: String,
    store_dir: String,
    session_db: String,
    index_db: String,
    session_present: bool,
    read_only: bool,
    timeout_secs: u64,
    index: Option<StoreStats>,
}

#[derive(Debug, Serialize)]
struct ContactSearchRow {
    source: String,
    name: Option<String>,
    full_name: Option<String>,
    first_name: Option<String>,
    push_name: Option<String>,
    jid: String,
    phone: Option<String>,
    lid: Option<String>,
    username: Option<String>,
    about: Option<String>,
}
