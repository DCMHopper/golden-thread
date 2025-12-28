import { invoke } from "@tauri-apps/api/core";
import type {
  AttachmentRow,
  MessageRow,
  MessageTags,
  ReactionSummary,
  ScrapbookMessage,
  SearchHit,
  Tag,
  ThreadMediaRow,
  ThreadSummary,
} from "./types";

export function listThreads(limit: number, offset: number) {
  return invoke<ThreadSummary[]>("list_threads_cmd", { limit, offset });
}

export function listMessages(threadId: string, beforeTs: number | null, beforeId: string | null, limit: number) {
  return invoke<MessageRow[]>("list_messages_cmd", {
    threadId,
    beforeTs,
    beforeId,
    limit,
  });
}

export function listMessagesAfter(threadId: string, afterTs: number, afterId: string | null, limit: number) {
  return invoke<MessageRow[]>("list_messages_after_cmd", {
    threadId,
    afterTs,
    afterId,
    limit,
  });
}

export function listMessagesAround(messageId: string, before: number, after: number) {
  return invoke<MessageRow[]>("list_messages_around_cmd", { messageId, before, after });
}

export function searchMessages(query: string, threadId: string | null, limit: number, offset: number) {
  return invoke<SearchHit[]>("search_messages_cmd", { query, threadId, limit, offset });
}

export function listThreadMedia(
  threadId: string,
  fromTs: number | null,
  toTs: number | null,
  sizeBucket: number | null,
  sort: string,
  limit: number,
  offset: number,
) {
  return invoke<ThreadMediaRow[]>("list_thread_media_cmd", {
    threadId,
    fromTs,
    toTs,
    sizeBucket,
    sort,
    limit,
    offset,
  });
}

export function listMessageAttachments(messageId: string) {
  return invoke<AttachmentRow[]>("list_message_attachments_cmd", { messageId });
}

export function listMessageReactions(messageIds: string[]) {
  return invoke<ReactionSummary[]>("list_message_reactions_cmd", { messageIds });
}

export function attachmentDataUrl(sha256: string, mime: string) {
  return invoke<string>("attachment_data_url_cmd", { sha256, mime });
}

export function attachmentPath(sha256: string, mime: string | null) {
  return invoke<string>("attachment_path_cmd", { sha256, mime });
}

export function attachmentThumbnail(sha256: string, mime: string | null, maxSize: number) {
  return invoke<string>("attachment_thumbnail_cmd", { sha256, mime, maxSize });
}

export function clearMediaCache() {
  return invoke<void>("clear_media_cache_cmd");
}

export function drainMediaEvictions() {
  return invoke<string[]>("drain_media_evictions_cmd");
}

export function listTags() {
  return invoke<Tag[]>("list_tags_cmd");
}

export function createTag(name: string, color: string) {
  return invoke<Tag>("create_tag_cmd", { name, color });
}

export function deleteTag(id: string) {
  return invoke<void>("delete_tag_cmd", { id });
}

export function getMessageTags(messageId: string) {
  return invoke<Tag[]>("get_message_tags_cmd", { messageId });
}

export function getMessageTagsBulk(messageIds: string[]) {
  return invoke<MessageTags[]>("get_message_tags_bulk_cmd", { messageIds });
}

export function setMessageTags(messageId: string, tagIds: string[]) {
  return invoke<void>("set_message_tags_cmd", { messageId, tagIds });
}

export function listScrapbookMessages(tagId: string, beforeTs: number | null, beforeId: string | null, limit: number) {
  return invoke<ScrapbookMessage[]>("list_scrapbook_messages_cmd", {
    tagId,
    beforeTs,
    beforeId,
    limit,
  });
}

export function seedDemo(primaryCount: number, secondaryThreads: number) {
  return invoke<void>("seed_demo_cmd", { primaryCount, secondaryThreads });
}

export function importBackup(path: string, passphrase: string) {
  return invoke<void>("import_backup_cmd", { path, passphrase });
}

export function resetArchive() {
  return invoke<void>("reset_archive_cmd");
}

export function getDiagnostics() {
  return invoke<string>("get_diagnostics_cmd");
}
