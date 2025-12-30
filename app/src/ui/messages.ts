/**
 * Message rendering, pagination, reactions, and viewport management.
 */

import {
  listMessages as apiListMessages,
  listMessagesAfter as apiListMessagesAfter,
  listMessageReactions as apiListMessageReactions,
  getMessageTagsBulk as apiGetMessageTagsBulk,
} from "./api";
import { ANCHOR_PADDING, PAGE_SIZE } from "./constants";
import {
  isTauri,
  messageStore,
  setMessageStore,
  messageById,
  currentThreadId,
  setCurrentThreadId,
  currentBeforeTs,
  setCurrentBeforeTs,
  currentBeforeId,
  setCurrentBeforeId,
  currentAfterTs,
  setCurrentAfterTs,
  currentAfterId,
  setCurrentAfterId,
  isLoadingMessages,
  setIsLoadingMessages,
  highlightMessageId,
  setHighlightMessageId,
  viewportAnchor,
  setViewportAnchor,
  anchorScheduled,
  setAnchorScheduled,
  messagesRequestId,
  incrementMessagesRequestId,
  createMessagesAbortController,
  searchQuery,
  searchMatchIds,
  isSearchJumping,
  reactionMap,
  threadScrollPositions,
  messageTagsCache,
  tagsStore,
} from "./state";
import { escapeHtml, highlightBody, messageSortTs, tagColorClass } from "./utils";
import type { MessageRow, ReactionSummary, Tag } from "./types";

let messageListEl: HTMLDivElement | null = null;

// Callbacks for cross-module communication
let onResetThreadState: ((threadId: string) => void) | null = null;
let onLoadAttachments: ((messageId: string, container: HTMLElement, mode: "button" | "auto") => void) | null = null;
let onLoadThreadMedia: ((threadId: string) => void) | null = null;
let onLoadGallery: ((reset: boolean) => void) | null = null;
let onShowTagPicker: ((messageId: string, triggerEl: HTMLElement) => void) | null = null;
let galleryViewEl: HTMLDivElement | null = null;
let requireMediaClickFn: (() => boolean) | null = null;

/**
 * Initialize the messages module with DOM elements and callbacks.
 */
export function initMessages(config: {
  messageList: HTMLDivElement | null;
  galleryView: HTMLDivElement | null;
  onResetThreadState: (threadId: string) => void;
  onLoadAttachments: (messageId: string, container: HTMLElement, mode: "button" | "auto") => void;
  onLoadThreadMedia: (threadId: string) => void;
  onLoadGallery: (reset: boolean) => void;
  onShowTagPicker: (messageId: string, triggerEl: HTMLElement) => void;
  requireMediaClick: () => boolean;
}) {
  messageListEl = config.messageList;
  galleryViewEl = config.galleryView;
  onResetThreadState = config.onResetThreadState;
  onLoadAttachments = config.onLoadAttachments;
  onLoadThreadMedia = config.onLoadThreadMedia;
  onLoadGallery = config.onLoadGallery;
  onShowTagPicker = config.onShowTagPicker;
  requireMediaClickFn = config.requireMediaClick;
}

/**
 * Resolves quote text for a message, checking messageById or metadata_json.
 */
export function resolveQuoteText(message: MessageRow): string | null {
  const quoteId = message.quote_message_id ?? null;
  if (quoteId) {
    const found = messageById.get(quoteId);
    if (found?.body) {
      return `"${found.body}"`;
    }
  }
  if (message.metadata_json) {
    try {
      const parsed = JSON.parse(message.metadata_json) as { quote_body?: string };
      if (parsed.quote_body) {
        return `"${parsed.quote_body}"`;
      }
    } catch {
      // ignore parse errors
    }
  }
  return null;
}

/**
 * Captures the current viewport anchor for scroll restoration.
 */
export function captureViewportAnchor() {
  if (!messageListEl) return;
  const scrollTop = messageListEl.scrollTop;
  let candidate = messageListEl.firstElementChild as HTMLElement | null;
  if (!candidate) return;
  let current = candidate;
  while (current) {
    if (current.offsetTop >= scrollTop) {
      candidate = current;
      break;
    }
    current = current.nextElementSibling as HTMLElement | null;
  }
  setViewportAnchor({
    id: candidate.dataset.messageId ?? "",
    offset: candidate.offsetTop - scrollTop,
  });
}

/**
 * Schedules a viewport anchor capture on next animation frame.
 */
export function scheduleAnchorCapture() {
  if (anchorScheduled) return;
  setAnchorScheduled(true);
  requestAnimationFrame(() => {
    setAnchorScheduled(false);
    captureViewportAnchor();
  });
}

/**
 * Restores the viewport to the captured anchor position.
 */
export function restoreViewportAnchor() {
  if (!messageListEl || !viewportAnchor || !viewportAnchor.id) return;
  const target = messageListEl.querySelector(
    `.message[data-message-id="${viewportAnchor.id}"]`,
  ) as HTMLElement | null;
  if (!target) return;
  messageListEl.scrollTop = Math.max(0, target.offsetTop - viewportAnchor.offset);
}

/**
 * Scrolls to put a message at the top of the viewport with padding.
 */
export function scrollToMessageTop(messageId: string, padding = ANCHOR_PADDING) {
  if (!messageListEl) return;
  const target = messageListEl.querySelector(
    `.message[data-message-id="${messageId}"]`,
  ) as HTMLElement | null;
  if (!target) return;
  requestAnimationFrame(() => {
    if (!messageListEl) return;
    const listRect = messageListEl.getBoundingClientRect();
    const msgRect = target.getBoundingClientRect();
    const delta = msgRect.top - listRect.top - padding;
    messageListEl.scrollTop = Math.max(0, messageListEl.scrollTop + delta);
  });
}

/**
 * Fetches and caches reactions for the given message IDs.
 */
export async function fetchReactionsForMessages(messageIds: string[]) {
  if (!isTauri || messageIds.length === 0) return;
  const missing = messageIds.filter((id) => !reactionMap.has(id));
  if (missing.length === 0) return;
  try {
    const summaries: ReactionSummary[] = await apiListMessageReactions(missing);
    const grouped = new Map<string, ReactionSummary[]>();
    summaries.forEach((item) => {
      const list = grouped.get(item.message_id) ?? [];
      list.push(item);
      grouped.set(item.message_id, list);
    });
    missing.forEach((id) => {
      if (!grouped.has(id)) {
        grouped.set(id, []);
      }
    });
    grouped.forEach((items, messageId) => {
      reactionMap.set(messageId, items);
      renderReactions(messageId, items);
    });
  } catch {
    // ignore
  }
}

/**
 * Renders reaction pills for a message.
 */
export function renderReactions(messageId: string, items: ReactionSummary[]) {
  if (!messageListEl) return;
  const container = messageListEl.querySelector(`.reactions[data-message-id="${messageId}"]`);
  if (!container) return;
  container.innerHTML = "";
  items.forEach((item) => {
    const pill = document.createElement("div");
    pill.className = "reaction-pill";
    pill.textContent = `${item.emoji} ${item.count}`;
    container.appendChild(pill);
  });
}

/**
 * Fetches and caches tags for the given message IDs.
 */
export async function fetchTagsForMessages(messageIds: string[]) {
  if (!isTauri || messageIds.length === 0) return;
  const missing = messageIds.filter((id) => !messageTagsCache.has(id));
  if (missing.length > 0) {
    try {
      const batches = await apiGetMessageTagsBulk(missing);
      const seen = new Set<string>();
      batches.forEach((entry) => {
        messageTagsCache.set(entry.message_id, entry.tags);
        seen.add(entry.message_id);
      });
      missing.forEach((id) => {
        if (!seen.has(id)) {
          messageTagsCache.set(id, []);
        }
      });
    } catch {
      // Ignore errors
    }
  }
  messageIds.forEach((id) => {
    const tags = messageTagsCache.get(id);
    if (tags) {
      renderMessageTags(id, tags);
    }
  });
}

/**
 * Renders tag dots for a message.
 */
export function renderMessageTags(messageId: string, tags: Tag[]) {
  const container = document.querySelector(`.tag-dots[data-message-id="${messageId}"]`);
  if (!container) return;
  const fragment = document.createDocumentFragment();
  tags.forEach((tag) => {
    const dot = document.createElement("span");
    dot.className = `tag-dot ${tagColorClass(tag.color)}`;
    dot.title = tag.name;
    fragment.appendChild(dot);
  });
  container.replaceChildren(fragment);
}

/**
 * Refreshes tag dots for all visible messages.
 */
export function refreshVisibleMessageTags() {
  document.querySelectorAll(".tag-dots").forEach((container) => {
    container.replaceChildren();
  });
  const visibleMessageIds = messageStore.map((m) => m.id);
  if (visibleMessageIds.length > 0) {
    void fetchTagsForMessages(visibleMessageIds);
  }
}

/**
 * Renders messages to the message list.
 */
export function renderMessages(messages: MessageRow[], mode: "replace" | "append" | "prepend") {
  if (!messageListEl) return;
  captureViewportAnchor();
  if (mode === "replace") {
    messageListEl.replaceChildren();
  }

  const fragment = document.createDocumentFragment();
  const requireClick = requireMediaClickFn?.() ?? true;

  messages.forEach((msg) => {
    const div = document.createElement("div");
    div.className = `message ${msg.is_outgoing ? "outgoing" : ""}`;
    if (msg.id === highlightMessageId) {
      div.classList.add("highlight");
    }
    div.dataset.messageId = msg.id;

    const ts = messageSortTs(msg);
    const tsLabel = ts ? new Date(ts).toLocaleString() : "";

    const quoteText = resolveQuoteText(msg);
    if (quoteText) {
      const quote = document.createElement("div");
      quote.className = "quote";
      quote.textContent = quoteText;
      div.appendChild(quote);
    }

    const body = document.createElement("div");
    const bodyText = msg.body ?? "(no text)";
    if (searchQuery && searchMatchIds.has(msg.id)) {
      body.innerHTML = highlightBody(bodyText, searchQuery);
      div.classList.add("match");
      if (msg.id === highlightMessageId) {
        div.classList.add("match-current");
      }
    } else {
      body.textContent = bodyText;
    }
    div.appendChild(body);

    if (tsLabel) {
      const time = document.createElement("div");
      time.className = "meta";
      time.textContent = tsLabel;
      div.appendChild(time);
    }

    const media = document.createElement("div");
    media.className = "media";
    div.appendChild(media);

    const reactions = document.createElement("div");
    reactions.className = "reactions";
    reactions.dataset.messageId = msg.id;
    div.appendChild(reactions);

    const tagContainer = document.createElement("div");
    tagContainer.className = "message-tags";
    const dotsContainer = document.createElement("div");
    dotsContainer.className = "tag-dots";
    dotsContainer.dataset.messageId = msg.id;
    tagContainer.appendChild(dotsContainer);
    const trigger = document.createElement("span");
    trigger.className = "message-tag-trigger";
    trigger.textContent = "\u{1F3F7}";
    trigger.title = "Tag this message";
    trigger.dataset.messageId = msg.id;
    tagContainer.appendChild(trigger);
    div.appendChild(tagContainer);

    if (!isSearchJumping) {
      if (msg.is_view_once) {
        media.textContent = "View-once media hidden";
      } else if (msg.id.startsWith("mms:")) {
        onLoadAttachments?.(msg.id, media, requireClick ? "button" : "auto");
      }
    }

    fragment.appendChild(div);
  });

  if (mode === "prepend") {
    messageListEl.prepend(fragment);
  } else {
    messageListEl.appendChild(fragment);
  }

  restoreViewportAnchor();
}

/**
 * Loads messages for a thread with pagination.
 */
export async function loadMessages(threadId: string, reset: boolean) {
  if (!isTauri || !messageListEl) return;
  if (isLoadingMessages) return;

  setIsLoadingMessages(true);
  const requestId = incrementMessagesRequestId();
  const abortController = createMessagesAbortController();
  // Capture expected values at request time to detect stale responses
  const expectedThreadId = threadId;

  try {
    if (reset) {
      onResetThreadState?.(threadId);
    }

    const messages: MessageRow[] = await apiListMessages(threadId, currentBeforeTs, currentBeforeId, PAGE_SIZE);

    // Check if this request was cancelled or superseded
    if (abortController.signal.aborted) return;
    // Verify the response is still relevant (user hasn't switched threads)
    if (requestId !== messagesRequestId || currentThreadId !== expectedThreadId) return;

    if (messages.length > 0) {
      const newestFirst = messages;
      const asc = [...newestFirst].reverse();
      const oldest = asc[0];
      const newest = asc[asc.length - 1];
      setCurrentBeforeTs(messageSortTs(oldest));
      setCurrentBeforeId(oldest.id);

      if (reset) {
        captureViewportAnchor();
        setMessageStore(asc);
        messageById.clear();
        asc.forEach((msg) => messageById.set(msg.id, msg));
        setCurrentAfterTs(messageSortTs(newest));
        setCurrentAfterId(newest.id);
        renderMessages(asc, "replace");
        reactionMap.clear();
        void fetchReactionsForMessages(asc.map((msg) => msg.id));
        void fetchTagsForMessages(asc.map((msg) => msg.id));
        onLoadThreadMedia?.(threadId);

        const savedScrollPos = threadScrollPositions.get(threadId);
        requestAnimationFrame(() => {
          if (messageListEl) {
            if (savedScrollPos !== undefined) {
              messageListEl.scrollTop = savedScrollPos;
            } else {
              messageListEl.scrollTop = messageListEl.scrollHeight;
            }
          }
        });

        if (galleryViewEl && !galleryViewEl.classList.contains("hidden")) {
          onLoadGallery?.(true);
        }
      } else {
        captureViewportAnchor();
        const prevHeight = messageListEl.scrollHeight;
        setMessageStore(asc.concat(messageStore));
        asc.forEach((msg) => messageById.set(msg.id, msg));
        renderMessages(asc, "prepend");
        void fetchReactionsForMessages(asc.map((msg) => msg.id));
        void fetchTagsForMessages(asc.map((msg) => msg.id));
        const delta = messageListEl.scrollHeight - prevHeight;
        messageListEl.scrollTop = messageListEl.scrollTop + delta;
      }
    }
  } finally {
    setIsLoadingMessages(false);
  }
}

/**
 * Loads newer messages for the current thread.
 */
export async function loadNewerMessages() {
  if (!isTauri || !messageListEl || !currentThreadId || !currentAfterTs) return;
  if (isLoadingMessages) return;

  setIsLoadingMessages(true);
  const requestId = incrementMessagesRequestId();
  const abortController = createMessagesAbortController();
  // Capture expected values at request time to detect stale responses
  const expectedThreadId = currentThreadId;

  try {
    const messages: MessageRow[] = await apiListMessagesAfter(
      expectedThreadId,
      currentAfterTs,
      currentAfterId,
      PAGE_SIZE,
    );

    // Check if this request was cancelled or superseded
    if (abortController.signal.aborted) return;
    if (requestId !== messagesRequestId || currentThreadId !== expectedThreadId) return;

    if (messages.length > 0) {
      const asc = messages;
      const newest = asc[asc.length - 1];
      setCurrentAfterTs(messageSortTs(newest));
      setCurrentAfterId(newest.id);
      setMessageStore(messageStore.concat(asc));
      asc.forEach((msg) => messageById.set(msg.id, msg));
      renderMessages(asc, "append");
      void fetchReactionsForMessages(asc.map((msg) => msg.id));
      void fetchTagsForMessages(asc.map((msg) => msg.id));
    }
  } finally {
    setIsLoadingMessages(false);
  }
}

/**
 * Gets the message list element.
 */
export function getMessageListEl(): HTMLDivElement | null {
  return messageListEl;
}

/**
 * Gets message store for external access.
 */
export function getMessageStore(): MessageRow[] {
  return messageStore;
}

/**
 * Click handler for the message list (delegation).
 */
export function handleMessageListClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (!target) return;

  const tagTrigger = target.closest(".message-tag-trigger") as HTMLElement | null;
  if (tagTrigger) {
    event.stopPropagation();
    const messageId = tagTrigger.dataset.messageId;
    if (messageId) {
      onShowTagPicker?.(messageId, tagTrigger);
    }
    return;
  }
}
