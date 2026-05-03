use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "whatshell",
    version,
    about = "WhatsApp Web CLI for coding agents"
)]
pub struct Cli {
    #[arg(long, global = true, env = "WHATSHELL_STORE")]
    pub store: Option<PathBuf>,

    #[arg(long, global = true)]
    pub json: bool,

    #[arg(long, global = true, default_value = "60")]
    pub timeout: u64,

    #[arg(long, global = true, env = "WHATSHELL_READ_ONLY")]
    pub read_only: bool,

    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Auth(AuthCommand),
    Sync(SyncCommand),
    Listen(ListenCommand),
    Send(SendCommand),
    Messages(MessagesCommand),
    Chats(ChatsCommand),
    Contacts(ContactsCommand),
    Groups(GroupsCommand),
    Media(MediaCommand),
    Presence(PresenceCommand),
    Profile(ProfileCommand),
    Blocking(BlockingCommand),
    Status(StatusCommand),
    Export(ExportCommand),
    Doctor(DoctorCommand),
}

#[derive(Debug, Args)]
pub struct AuthCommand {
    #[command(subcommand)]
    pub command: Option<AuthSubcommand>,

    #[arg(long)]
    pub phone: Option<String>,

    #[arg(long)]
    pub code: Option<String>,

    #[arg(long, default_value = "20")]
    pub idle_exit: u64,

    #[arg(long)]
    pub follow: bool,
}

#[derive(Debug, Subcommand)]
pub enum AuthSubcommand {
    Status,
    Logout,
}

#[derive(Debug, Args)]
pub struct SyncCommand {
    #[arg(long)]
    pub once: bool,

    #[arg(long)]
    pub follow: bool,

    #[arg(long, default_value = "15")]
    pub idle_exit: u64,

    #[arg(long)]
    pub stream_jsonl: bool,
}

#[derive(Debug, Args)]
pub struct ListenCommand {
    #[arg(long)]
    pub stream_jsonl: bool,
}

#[derive(Debug, Args)]
pub struct SendCommand {
    #[command(subcommand)]
    pub command: SendSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SendSubcommand {
    Text(SendText),
    File(SendFile),
    React(SendReact),
    Poll(SendPoll),
    Location(SendLocation),
    Contact(SendContact),
}

#[derive(Debug, Args)]
pub struct SendText {
    #[arg(long)]
    pub to: String,

    #[arg(short, long)]
    pub message: String,

    #[arg(long)]
    pub reply_to: Option<String>,

    #[arg(long)]
    pub reply_sender: Option<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct SendFile {
    #[arg(long)]
    pub to: String,

    #[arg(long)]
    pub file: PathBuf,

    #[arg(long)]
    pub caption: Option<String>,

    #[arg(long)]
    pub mime: Option<String>,

    #[arg(long)]
    pub filename: Option<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct SendReact {
    #[arg(long)]
    pub to: String,

    #[arg(long)]
    pub id: String,

    #[arg(long, default_value = "👍")]
    pub reaction: String,

    #[arg(long)]
    pub sender: Option<String>,

    #[arg(long)]
    pub from_me: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct SendPoll {
    #[arg(long)]
    pub to: String,

    #[arg(short, long)]
    pub question: String,

    #[arg(short = 'o', long = "option", required = true)]
    pub options: Vec<String>,

    #[arg(long, default_value = "1")]
    pub selectable_count: u32,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct SendLocation {
    #[arg(long)]
    pub to: String,

    #[arg(long)]
    pub latitude: f64,

    #[arg(long)]
    pub longitude: f64,

    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub address: Option<String>,

    #[arg(long)]
    pub url: Option<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct SendContact {
    #[arg(long)]
    pub to: String,

    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub phone: String,

    #[arg(long)]
    pub vcard: Option<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MessagesCommand {
    #[command(subcommand)]
    pub command: MessagesSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum MessagesSubcommand {
    List(MessageList),
    Search(MessageSearch),
    Show(MessageShow),
    Context(MessageContext),
    Reply(MessageReply),
    Edit(MessageEdit),
    Revoke(MessageRevoke),
    DeleteForMe(MessageDeleteForMe),
    Star(MessageTarget),
    Unstar(MessageTarget),
}

#[derive(Debug, Args)]
pub struct MessageList {
    #[arg(long)]
    pub chat: Option<String>,

    #[arg(long)]
    pub sender: Option<String>,

    #[arg(long)]
    pub from_me: bool,

    #[arg(long)]
    pub from_them: bool,

    #[arg(long)]
    pub r#type: Option<MediaFilter>,

    #[arg(long, default_value = "50")]
    pub limit: usize,

    #[arg(long)]
    pub asc: bool,
}

#[derive(Debug, Args)]
pub struct MessageSearch {
    pub query: String,

    #[arg(long)]
    pub chat: Option<String>,

    #[arg(long)]
    pub sender: Option<String>,

    #[arg(long)]
    pub r#type: Option<MediaFilter>,

    #[arg(long, default_value = "25")]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct MessageShow {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,
}

#[derive(Debug, Args)]
pub struct MessageContext {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,

    #[arg(long, default_value = "5")]
    pub before: usize,

    #[arg(long, default_value = "5")]
    pub after: usize,
}

#[derive(Debug, Args)]
pub struct MessageReply {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,

    #[arg(short, long)]
    pub message: String,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MessageEdit {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,

    #[arg(short, long)]
    pub message: String,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MessageRevoke {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,

    #[arg(long)]
    pub admin_sender: Option<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MessageDeleteForMe {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,

    #[arg(long)]
    pub delete_media: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MessageTarget {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum MediaFilter {
    Text,
    Image,
    Video,
    Audio,
    Document,
    Sticker,
    Reaction,
    Poll,
    Location,
    Contact,
    Protocol,
}

impl MediaFilter {
    pub fn as_store_value(&self) -> Option<&'static str> {
        match self {
            Self::Text => Some("text"),
            Self::Image => Some("image"),
            Self::Video => Some("video"),
            Self::Audio => Some("audio"),
            Self::Document => Some("document"),
            Self::Sticker => Some("sticker"),
            Self::Reaction => Some("reaction"),
            Self::Poll => Some("poll"),
            Self::Location => Some("location"),
            Self::Contact => Some("contact"),
            Self::Protocol => Some("protocol"),
        }
    }
}

#[derive(Debug, Args)]
pub struct ChatsCommand {
    #[command(subcommand)]
    pub command: ChatsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ChatsSubcommand {
    List(ChatList),
    Archive(ChatTarget),
    Unarchive(ChatTarget),
    Pin(ChatTarget),
    Unpin(ChatTarget),
    Mute(ChatTarget),
    Unmute(ChatTarget),
    MarkRead(ChatRead),
    Delete(ChatDelete),
}

#[derive(Debug, Args)]
pub struct ChatList {
    #[arg(long)]
    pub query: Option<String>,

    #[arg(long, default_value = "50")]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct ContactsCommand {
    #[command(subcommand)]
    pub command: ContactsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ContactsSubcommand {
    Sync(ContactSync),
    List(ContactList),
    Search(ContactSearch),
    Check(ContactPhones),
    Info(ContactPhones),
}

#[derive(Debug, Args)]
pub struct ContactSync {}

#[derive(Debug, Args)]
pub struct ContactList {
    #[arg(long)]
    pub query: Option<String>,

    #[arg(long, default_value = "50")]
    pub limit: usize,

    /// Read the local whatshell contact index without first syncing WhatsApp app state.
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct ContactSearch {
    pub query: String,

    #[arg(long, default_value = "25")]
    pub limit: usize,

    /// Read the local whatshell contact index without first syncing WhatsApp app state.
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct ContactPhones {
    #[arg(required = true)]
    pub phones: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ChatTarget {
    #[arg(long)]
    pub chat: String,
}

#[derive(Debug, Args)]
pub struct ChatRead {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub unread: bool,
}

#[derive(Debug, Args)]
pub struct ChatDelete {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub delete_media: bool,
}

#[derive(Debug, Args)]
pub struct GroupsCommand {
    #[command(subcommand)]
    pub command: GroupsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum GroupsSubcommand {
    List(GroupList),
    Info(GroupTarget),
    Create(GroupCreate),
    SetSubject(GroupSubjectArgs),
    SetDescription(GroupDescriptionArgs),
    InviteLink(GroupInviteLink),
    InviteInfo(GroupJoin),
    Leave(GroupTarget),
    Add(GroupParticipants),
    Remove(GroupParticipants),
    Promote(GroupParticipants),
    Demote(GroupParticipants),
    Lock(GroupTarget),
    Unlock(GroupTarget),
    Announce(GroupTarget),
    Unannounce(GroupTarget),
    Ephemeral(GroupEphemeral),
    Approval(GroupApproval),
    MemberAdd(GroupMemberAdd),
    Requests(GroupTarget),
    Approve(GroupParticipants),
    Reject(GroupParticipants),
}

#[derive(Debug, Args)]
pub struct GroupList {
    #[arg(long)]
    pub query: Option<String>,
}

#[derive(Debug, Args)]
pub struct GroupTarget {
    #[arg(long)]
    pub jid: String,
}

#[derive(Debug, Args)]
pub struct GroupCreate {
    #[arg(long)]
    pub subject: String,

    #[arg(long = "participant")]
    pub participants: Vec<String>,

    #[arg(long)]
    pub admins_only_add: bool,

    #[arg(long)]
    pub approval: bool,

    #[arg(long, default_value = "0")]
    pub ephemeral_seconds: u32,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct GroupSubjectArgs {
    #[arg(long)]
    pub jid: String,

    #[arg(long)]
    pub subject: String,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct GroupDescriptionArgs {
    #[arg(long)]
    pub jid: String,

    #[arg(long)]
    pub description: Option<String>,

    #[arg(long)]
    pub clear: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct GroupInviteLink {
    #[arg(long)]
    pub jid: String,

    #[arg(long)]
    pub reset: bool,
}

#[derive(Debug, Args)]
pub struct GroupJoin {
    pub code: String,
}

#[derive(Debug, Args)]
pub struct GroupParticipants {
    #[arg(long)]
    pub jid: String,

    #[arg(required = true)]
    pub participants: Vec<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct GroupEphemeral {
    #[arg(long)]
    pub jid: String,

    #[arg(long)]
    pub seconds: u32,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct GroupApproval {
    #[arg(long)]
    pub jid: String,

    #[arg(long)]
    pub on: bool,

    #[arg(long)]
    pub off: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct GroupMemberAdd {
    #[arg(long)]
    pub jid: String,

    #[arg(long)]
    pub admins_only: bool,

    #[arg(long)]
    pub everyone: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MediaCommand {
    #[command(subcommand)]
    pub command: MediaSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum MediaSubcommand {
    List(MediaList),
    Download(MediaDownload),
}

#[derive(Debug, Args)]
pub struct MediaList {
    #[arg(long)]
    pub chat: Option<String>,

    #[arg(long)]
    pub r#type: Option<MediaFilter>,

    #[arg(long, default_value = "50")]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct MediaDownload {
    #[arg(long)]
    pub chat: String,

    #[arg(long)]
    pub id: String,

    #[arg(short, long)]
    pub output: Option<PathBuf>,

    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
pub struct PresenceCommand {
    #[command(subcommand)]
    pub command: PresenceSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum PresenceSubcommand {
    Online,
    Offline,
    Typing(ChatTarget),
    Recording(ChatTarget),
    Paused(ChatTarget),
}

#[derive(Debug, Args)]
pub struct ProfileCommand {
    #[command(subcommand)]
    pub command: ProfileSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ProfileSubcommand {
    SetName(ProfileText),
    SetAbout(ProfileText),
    SetPicture(ProfilePicture),
    RemovePicture,
}

#[derive(Debug, Args)]
pub struct ProfileText {
    pub text: String,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ProfilePicture {
    pub file: PathBuf,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct BlockingCommand {
    #[command(subcommand)]
    pub command: BlockingSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum BlockingSubcommand {
    List,
    Block(ContactTarget),
    Unblock(ContactTarget),
    IsBlocked(ContactTarget),
}

#[derive(Debug, Args)]
pub struct ContactTarget {
    pub contact: String,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct StatusCommand {
    #[command(subcommand)]
    pub command: StatusSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum StatusSubcommand {
    Text(StatusText),
    Image(StatusMedia),
    Video(StatusVideo),
    Revoke(StatusRevoke),
}

#[derive(Debug, Args)]
pub struct StatusText {
    #[arg(short, long)]
    pub message: String,

    #[arg(long = "to", required = true)]
    pub recipients: Vec<String>,

    #[arg(long, default_value = "0xFF1E6E4F")]
    pub background: String,

    #[arg(long, default_value = "0")]
    pub font: i32,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct StatusMedia {
    #[arg(long)]
    pub file: PathBuf,

    #[arg(long)]
    pub thumbnail: Option<PathBuf>,

    #[arg(long)]
    pub caption: Option<String>,

    #[arg(long = "to", required = true)]
    pub recipients: Vec<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct StatusVideo {
    #[arg(long)]
    pub file: PathBuf,

    #[arg(long)]
    pub thumbnail: Option<PathBuf>,

    #[arg(long, default_value = "0")]
    pub duration: u32,

    #[arg(long)]
    pub caption: Option<String>,

    #[arg(long = "to", required = true)]
    pub recipients: Vec<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct StatusRevoke {
    #[arg(long)]
    pub id: String,

    #[arg(long = "to", required = true)]
    pub recipients: Vec<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ExportCommand {
    #[command(subcommand)]
    pub command: ExportSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ExportSubcommand {
    Messages(ExportMessages),
    Analytics(ExportAnalytics),
}

#[derive(Debug, Args)]
pub struct ExportMessages {
    #[arg(long)]
    pub chat: Option<String>,

    #[arg(long)]
    pub sender: Option<String>,

    #[arg(long)]
    pub from_me: bool,

    #[arg(long)]
    pub from_them: bool,

    #[arg(long)]
    pub r#type: Option<MediaFilter>,

    #[arg(long, default_value = "1000")]
    pub limit: usize,

    #[arg(long, value_enum, default_value = "json")]
    pub format: ExportFormat,

    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ExportAnalytics {
    #[arg(long)]
    pub chat: Option<String>,

    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ExportFormat {
    Json,
    Jsonl,
    Csv,
}

#[derive(Debug, Args)]
pub struct DoctorCommand {
    #[arg(long)]
    pub connect: bool,
}
