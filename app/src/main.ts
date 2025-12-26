import "./styles.css";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import flatpickr from "flatpickr";
import "flatpickr/dist/flatpickr.min.css";
import type {
  AttachmentRow,
  MediaAsset,
  MessageRow,
  MessageTags,
  ReactionSummary,
  ScrapbookMessage,
  SearchHit,
  SourceInfo,
  Tag,
  ThreadMediaRow,
  ThreadSummary,
} from "./ui/types";
import {
  attachmentDataUrl,
  attachmentPath,
  attachmentThumbnail,
  createTag as apiCreateTag,
  deleteTag as apiDeleteTag,
  getDiagnostics as apiGetDiagnostics,
  getMessageTags as apiGetMessageTags,
  getMessageTagsBulk as apiGetMessageTagsBulk,
  importBackup as apiImportBackup,
  listMessageAttachments as apiListMessageAttachments,
  listMessageReactions as apiListMessageReactions,
  listMessages as apiListMessages,
  listMessagesAfter as apiListMessagesAfter,
  listMessagesAround as apiListMessagesAround,
  listScrapbookMessages as apiListScrapbookMessages,
  listTags as apiListTags,
  listThreadMedia as apiListThreadMedia,
  listThreads as apiListThreads,
  resetArchive as apiResetArchive,
  searchMessages as apiSearchMessages,
  seedDemo as apiSeedDemo,
  setMessageTags as apiSetMessageTags,
} from "./ui/api";
import { getDom } from "./ui/dom";
import {
  ANCHOR_PADDING,
  ATTACHMENT_DATA_URL_CACHE_MAX,
  ATTACHMENT_FILE_URL_CACHE_MAX,
  ATTACHMENT_LIST_CACHE_MAX,
  ATTACHMENT_THUMB_CACHE_MAX,
  GALLERY_PAGE,
  PAGE_SIZE,
} from "./ui/constants";
import { LruCache } from "./ui/cache";
import {
  createMediaPlaceholder,
  debounce,
  escapeHtml,
  highlightBody,
  mediaSortTs,
  messageSortTs,
  parseDateFromInput,
  setMediaSource,
  throttleRaf,
  sizeBucket,
} from "./ui/utils";

const {
  statusEl,
  threadList,
  threadSearchInput,
  messageList,
  importBtn,
  importPanel,
  chooseFileBtn,
  importFileEl,
  passphraseInput,
  passphraseCount,
  runImportBtn,
  seedBtn,
  resetBtn,
  copyDiagBtn,
  searchInput,
  searchPrevBtn,
  searchNextBtn,
  searchControlsWrapper,
  searchCounter,
  searchAllBtn,
  searchResults,
  jumpDateInput,
  jumpBtn,
  jumpControls,
  jumpToggle,
  optionsBtn,
  optionsMenu,
  mediaToggle,
  darkModeToggle,
  tabMessages,
  tabGallery,
  messagesView,
  searchView,
  galleryView,
  galleryGrid,
  galleryFrom,
  galleryTo,
  galleryFromClear,
  galleryToClear,
  gallerySize,
  gallerySort,
  tabScrapbook,
  scrapbookView,
  scrapbookTagSelect,
  manageTagsBtn,
  scrapbookMessageList,
  tagManagerModal,
  closeTagManager,
  newTagName,
  tagColorPresets,
  createTagBtn,
  tagList,
} = getDom();
let selectedBackupPath: string | null = null;
let isBusy = false;
let currentThreadId: string | null = null;
let currentBeforeTs: number | null = null;
let currentBeforeId: string | null = null;
let currentAfterTs: number | null = null;
let currentAfterId: string | null = null;
let isLoadingMessages = false;
let highlightMessageId: string | null = null;
let searchHits: SearchHit[] = [];
let searchIndex = -1;
let searchQuery = "";
let searchMatchIds = new Set<string>();
let viewportAnchor: { id: string; offset: number } | null = null;
let anchorScheduled = false;
let isSearchJumping = false;
const attachmentCache = new LruCache<string, AttachmentRow[]>(ATTACHMENT_LIST_CACHE_MAX);
const attachmentDataCache = new LruCache<string, string>(ATTACHMENT_DATA_URL_CACHE_MAX);
const attachmentFileCache = new LruCache<string, string>(ATTACHMENT_FILE_URL_CACHE_MAX);
const attachmentThumbCache = new LruCache<string, string>(ATTACHMENT_THUMB_CACHE_MAX);
let requireMediaClick = true;
let messageStore: MessageRow[] = [];
let threadStore: ThreadSummary[] = [];
const reactionMap = new Map<string, ReactionSummary[]>();
const threadScrollPositions = new Map<string, number>();
let lightboxGallery: MediaAsset[] = [];
let lightboxIndex = -1;
let isLightboxFullscreen = false;
const messageAttachmentsCache = new Map<string, MediaAsset[]>();
const threadMediaCache = new Map<string, MediaAsset[]>();
let galleryItems: ThreadMediaRow[] = [];
let galleryOffset = 0;
let galleryLoading = false;
let galleryHasMore = true;
const galleryFilterReload = debounce(() => loadGallery(true), 200);
const threadFilterReload = debounce(() => applyThreadFilters(), 200);
const searchDebounced = debounce(() => runSearch(), 250);
let tagsStore: Tag[] = [];
let messageTagsCache = new Map<string, Tag[]>();
let currentScrapbookTagId: string | null = null;
let scrapbookBeforeTs: number | null = null;
let scrapbookBeforeId: string | null = null;
let scrapbookMessages: ScrapbookMessage[] = [];
let isLoadingScrapbook = false;
const scrapbookScrollPositions = new Map<string, number>();
const attachmentsById = new Map<string, MediaAsset>();
let messagesRequestId = 0;
let searchRequestId = 0;
let galleryRequestId = 0;
let scrapbookRequestId = 0;
let resetConfirmState: "initial" | "confirming" = "initial";
let resetConfirmTimeout: number | null = null;
let deleteTagConfirmId: string | null = null;
let deleteTagConfirmTimeout: number | null = null;
let selectedTagColor = "#c3684a";
let activeThreadId: string | null = null;
let activeThreadEl: HTMLLIElement | null = null;
const messageById = new Map<string, MessageRow>();

const TAG_COLOR_PRESETS = [
  "#c3684a",
  "#d3a24b",
  "#9b7a52",
  "#5f8f6b",
  "#5f88b3",
  "#b36a78",
  "#7a6bb0",
  "#4f8f86",
];

const isTauri = typeof (window as any).__TAURI_INTERNALS__ !== "undefined";
if (!isTauri && statusEl) {
  statusEl.textContent = "Tauri runtime not available. Open via `npm run tauri dev`.";
}
if (isTauri) {
  listen<string>("import_status", (event) => {
    if (statusEl) statusEl.textContent = event.payload;
  }).catch(() => {});
}

if (mediaToggle) {
  const stored = localStorage.getItem("gt_media_click");
  requireMediaClick = stored ? stored === "true" : true;
  mediaToggle.setAttribute("aria-checked", String(requireMediaClick));
}

if (darkModeToggle) {
  const savedTheme = localStorage.getItem("gt_dark_mode");
  const isDark = savedTheme === "true";
  document.documentElement.setAttribute("data-theme", isDark ? "dark" : "light");
  darkModeToggle.setAttribute("aria-checked", String(isDark));
}

const savedAccent = localStorage.getItem("gt_accent_color") || "amber";
document.documentElement.setAttribute("data-accent", savedAccent);
document.querySelectorAll(".color-swatch").forEach(swatch => {
  const color = (swatch as HTMLElement).dataset.color;
  const isActive = color === savedAccent;
  swatch.classList.toggle("active", isActive);
  swatch.setAttribute("aria-checked", String(isActive));
});

function setBusy(state: boolean, message?: string) {
  isBusy = state;
  const disabled = state;
  [importBtn, chooseFileBtn, runImportBtn, seedBtn, resetBtn, passphraseInput].forEach((el) => {
    if (!el) return;
    (el as HTMLButtonElement | HTMLInputElement).disabled = disabled;
  });
  if (statusEl && message) {
    statusEl.textContent = message;
  }
}

function resetGalleryState() {
  galleryOffset = 0;
  galleryHasMore = true;
  galleryLoading = false;
  galleryItems = [];
  if (galleryGrid) {
    galleryGrid.replaceChildren();
  }
}

function resetThreadState(threadId: string) {
  if (currentThreadId && currentThreadId !== threadId && messageList) {
    threadScrollPositions.set(currentThreadId, messageList.scrollTop);
  }
  currentThreadId = threadId;
  currentBeforeTs = null;
  currentBeforeId = null;
  currentAfterTs = null;
  currentAfterId = null;
  highlightMessageId = null;
  messageById.clear();
  resetGalleryState();
}

function resetArchiveState() {
  attachmentCache.clear();
  attachmentDataCache.clear();
  attachmentFileCache.clear();
  attachmentThumbCache.clear();
  attachmentsById.clear();
  threadMediaCache.clear();
  reactionMap.clear();
  messageStore = [];
  currentThreadId = null;
  activeThreadId = null;
  activeThreadEl = null;
  messageById.clear();
  currentBeforeTs = null;
  currentBeforeId = null;
  currentAfterTs = null;
  currentAfterId = null;
  highlightMessageId = null;
  resetGalleryState();
  if (messageList) messageList.replaceChildren();
  if (searchResults) searchResults.classList.add("hidden");
}

async function refreshThreads() {
  if (!isTauri || !threadList || !messageList) return;
  const threads: ThreadSummary[] = await apiListThreads(500, 0);
  threadStore = threads;
  if (!threads.length) {
    const empty = document.createElement("li");
    empty.textContent = "No threads yet. Import a backup.";
    threadList.replaceChildren(empty);
    messageList.replaceChildren();
    currentThreadId = null;
    currentBeforeTs = null;
    currentBeforeId = null;
    currentAfterTs = null;
    currentAfterId = null;
    return;
  }
  applyThreadFilters();
}

function renderThreadList(threads: ThreadSummary[]) {
  if (!threadList) return;
  activeThreadEl = null;
  threadList.replaceChildren();
  const currentActiveId = activeThreadId ?? currentThreadId;
  threads.forEach((thread, idx) => {
    const li = document.createElement("li");
    li.textContent = `${thread.name ?? "(unnamed)"} · ${thread.message_count} messages`;
    li.dataset.threadId = thread.id;
    if (idx === 0 && !currentThreadId) {
      li.classList.add("active");
      activeThreadId = thread.id;
      activeThreadEl = li;
      void loadMessages(thread.id, true);
    }
    if (thread.id === currentActiveId) {
      li.classList.add("active");
      activeThreadId = thread.id;
      activeThreadEl = li;
    }
    li.addEventListener("click", () => {
      if (activeThreadEl && activeThreadEl !== li) {
        activeThreadEl.classList.remove("active");
      }
      li.classList.add("active");
      activeThreadEl = li;
      activeThreadId = thread.id;
      clearSearchState();
      void loadMessages(thread.id, true);
    });
    threadList.appendChild(li);
  });
}

function applyThreadFilters() {
  const query = threadSearchInput?.value?.trim().toLowerCase() ?? "";
  let filtered = threadStore.slice();
  if (query) {
    filtered = filtered.filter((thread) => (thread.name ?? "").toLowerCase().includes(query));
  }
  filtered.sort((a, b) => (b.last_message_at ?? 0) - (a.last_message_at ?? 0));
  renderThreadList(filtered);
}

function resolveQuoteText(message: MessageRow): string | null {
  const quoteId = message.quote_message_id ?? null;
  if (quoteId) {
    const found = messageById.get(quoteId);
    if (found?.body) {
      return `“${found.body}”`;
    }
  }
  if (message.metadata_json) {
    try {
      const parsed = JSON.parse(message.metadata_json) as { quote_body?: string };
      if (parsed.quote_body) {
        return `“${parsed.quote_body}”`;
      }
    } catch {
      // ignore parse errors
    }
  }
  return null;
}

function captureViewportAnchor() {
  if (!messageList) return;
  const scrollTop = messageList.scrollTop;
  let candidate = messageList.firstElementChild as HTMLElement | null;
  if (!candidate) return;
  let current = candidate;
  while (current) {
    if (current.offsetTop >= scrollTop) {
      candidate = current;
      break;
    }
    current = current.nextElementSibling as HTMLElement | null;
  }
  viewportAnchor = {
    id: candidate.dataset.messageId ?? "",
    offset: candidate.offsetTop - scrollTop,
  };
}

function scheduleAnchorCapture() {
  if (anchorScheduled) return;
  anchorScheduled = true;
  requestAnimationFrame(() => {
    anchorScheduled = false;
    captureViewportAnchor();
  });
}

function restoreViewportAnchor() {
  if (!messageList || !viewportAnchor || !viewportAnchor.id) return;
  const target = messageList.querySelector(
    `.message[data-message-id="${viewportAnchor.id}"]`,
  ) as HTMLElement | null;
  if (!target) return;
  messageList.scrollTop = Math.max(0, target.offsetTop - viewportAnchor.offset);
}

function scrollToMessageTop(messageId: string, padding = ANCHOR_PADDING) {
  if (!messageList) return;
  const target = messageList.querySelector(
    `.message[data-message-id="${messageId}"]`,
  ) as HTMLElement | null;
  if (!target) return;
  requestAnimationFrame(() => {
    if (!messageList) return;
    const listRect = messageList.getBoundingClientRect();
    const msgRect = target.getBoundingClientRect();
    const delta = msgRect.top - listRect.top - padding;
    messageList.scrollTop = Math.max(0, messageList.scrollTop + delta);
  });
}

function updateSearchControls() {
  const total = searchHits.length;
  const index = total ? searchIndex + 1 : 0;
  if (searchCounter) {
    searchCounter.textContent = `${index}/${total}`;
  }
  if (searchPrevBtn) {
    searchPrevBtn.disabled = total === 0;
  }
  if (searchNextBtn) {
    searchNextBtn.disabled = total === 0;
  }
  if (searchControlsWrapper) {
    searchControlsWrapper.classList.toggle("hidden", total === 0);
  }
  if (searchAllBtn) {
    if (total > 0) {
      searchAllBtn.classList.remove("hidden");
    } else {
      searchAllBtn.classList.add("hidden");
    }
  }
  if (jumpControls && !jumpControls.classList.contains("hidden") && total === 0) {
    jumpControls.classList.add("hidden");
  }
}

async function jumpToHit(hit: SearchHit) {
  if (!isTauri) return;
  highlightMessageId = hit.message.id;
  currentThreadId = hit.message.thread_id;
  document.querySelectorAll("#thread-list li").forEach((el) => el.classList.remove("active"));
  const threadEl = document.querySelector(`#thread-list li[data-thread-id="${hit.message.thread_id}"]`);
  if (threadEl) {
    threadEl.classList.add("active");
  }
  if (messageStore.some((msg) => msg.id === hit.message.id)) {
    viewportAnchor = { id: hit.message.id, offset: 0 };
    scrollToMessageTop(hit.message.id);
    return;
  }
  isSearchJumping = true;
  const context: MessageRow[] = await apiListMessagesAround(hit.message.id, 40, 40);
  const asc = context;
  const oldest = asc[0];
  const newest = asc[asc.length - 1];
  currentBeforeTs = oldest ? messageSortTs(oldest) : null;
  currentBeforeId = oldest ? oldest.id : null;
  currentAfterTs = newest ? messageSortTs(newest) : null;
  currentAfterId = newest ? newest.id : null;
  messageStore = asc;
  viewportAnchor = { id: hit.message.id, offset: 0 };
  renderMessages(asc, "replace");
  reactionMap.clear();
  void fetchReactionsForMessages(asc.map((msg) => msg.id));
  scrollToMessageTop(hit.message.id);
  isSearchJumping = false;
}

async function runSearch() {
  if (!isTauri) return;
  const requestId = ++searchRequestId;
  const query = searchInput?.value?.trim() ?? "";
  searchQuery = query;
  if (!query) {
    clearSearchState();
    if (messageList && messageStore.length) {
      captureViewportAnchor();
      renderMessages(messageStore, "replace");
      restoreViewportAnchor();
    }
    if (searchResults) {
      searchResults.classList.add("hidden");
      searchResults.replaceChildren();
    }
    return;
  }
  if (statusEl) statusEl.textContent = "Searching...";
  try {
    const hits: SearchHit[] = await apiSearchMessages(query, currentThreadId, 200, 0);
    if (requestId !== searchRequestId) return;
    const sorted = hits.sort(
      (a, b) =>
        messageSortTs(b.message) - messageSortTs(a.message) || a.message.id.localeCompare(b.message.id),
    );
    searchHits = sorted;
    searchMatchIds = new Set(sorted.map((hit) => hit.message.id));
    if (!searchHits.length) {
      searchIndex = -1;
      updateSearchControls();
      if (statusEl) statusEl.textContent = "No matches.";
      return;
    }
    searchIndex = 0;
    if (searchControlsWrapper) {
      searchControlsWrapper.classList.remove("hidden");
    }
    updateSearchControls();
    await jumpToHit(searchHits[0]);
    if (statusEl) statusEl.textContent = `Found ${searchHits.length} matches.`;
  } catch (err) {
    if (statusEl) statusEl.textContent = `Search failed: ${err}`;
  }
}

function renderSearchResultsList(hits: SearchHit[]) {
  if (!searchResults) return;
  searchResults.replaceChildren();
  const header = document.createElement("div");
  header.className = "result";
  const title = document.createElement("div");
  title.textContent = `Search results (${hits.length})`;
  const back = document.createElement("button");
  back.className = "secondary";
  back.textContent = "Back to thread";
  back.addEventListener("click", () => {
    setContentPane("messages");
  });
  header.appendChild(title);
  header.appendChild(back);
  searchResults.appendChild(header);
  hits.forEach((hit) => {
    const div = document.createElement("div");
    div.className = "result";
    div.innerHTML = `<div>${escapeHtml(hit.message.body ?? "(no text)")}</div>`;
    const meta = document.createElement("div");
    meta.className = "meta";
    const ts = messageSortTs(hit.message);
    meta.textContent = `${ts ? new Date(ts).toLocaleString() : ""} · Thread ${hit.message.thread_id}`;
    div.appendChild(meta);
    div.addEventListener("click", async () => {
      searchResults.classList.add("hidden");
      if (messageList) messageList.classList.remove("hidden");
      await jumpToHit(hit);
    });
    searchResults.appendChild(div);
  });
  searchResults.classList.remove("hidden");
  setContentPane("search");
}
async function fetchReactionsForMessages(messageIds: string[]) {
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

function renderReactions(messageId: string, items: ReactionSummary[]) {
  if (!messageList) return;
  const container = messageList.querySelector(`.reactions[data-message-id="${messageId}"]`);
  if (!container) return;
  container.innerHTML = "";
  items.forEach((item) => {
    const pill = document.createElement("div");
    pill.className = "reaction-pill";
    pill.textContent = `${item.emoji} ${item.count}`;
    container.appendChild(pill);
  });
}

function renderMessages(messages: MessageRow[], mode: "replace" | "append" | "prepend") {
  if (!messageList) return;
  captureViewportAnchor();
  if (mode === "replace") {
    messageList.replaceChildren();
  }
  const fragment = document.createDocumentFragment();
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
    trigger.textContent = "🏷";
    trigger.title = "Tag this message";
    trigger.dataset.messageId = msg.id;
    tagContainer.appendChild(trigger);
    div.appendChild(tagContainer);
    if (!isSearchJumping) {
      if (msg.is_view_once) {
        media.textContent = "View-once media hidden";
      } else if (msg.id.startsWith("mms:")) {
        if (requireMediaClick) {
          void loadAttachments(msg.id, media, "button");
        } else {
          void loadAttachments(msg.id, media, "auto");
        }
      }
    }
    fragment.appendChild(div);
  });
  if (mode === "prepend") {
    messageList.prepend(fragment);
  } else {
    messageList.appendChild(fragment);
  }
  restoreViewportAnchor();
}

async function loadMessages(threadId: string, reset: boolean) {
  if (!isTauri || !messageList) return;
  if (isLoadingMessages) return;
  isLoadingMessages = true;
  const requestId = ++messagesRequestId;
  try {
    if (reset) {
      resetThreadState(threadId);
    }
    const messages: MessageRow[] = await apiListMessages(threadId, currentBeforeTs, currentBeforeId, PAGE_SIZE);
    if (requestId !== messagesRequestId || currentThreadId !== threadId) return;
    if (messages.length > 0) {
      const newestFirst = messages;
      const asc = [...newestFirst].reverse();
      const oldest = asc[0];
      const newest = asc[asc.length - 1];
      currentBeforeTs = messageSortTs(oldest);
      currentBeforeId = oldest.id;
      if (reset) {
        captureViewportAnchor();
        messageStore = asc;
        messageById.clear();
        asc.forEach((msg) => messageById.set(msg.id, msg));
        currentAfterTs = messageSortTs(newest);
        currentAfterId = newest.id;
        renderMessages(asc, "replace");
        reactionMap.clear();
        void fetchReactionsForMessages(asc.map((msg) => msg.id));
        void fetchTagsForMessages(asc.map((msg) => msg.id));
        void loadThreadMedia(threadId);
        const savedScrollPos = threadScrollPositions.get(threadId);
        requestAnimationFrame(() => {
          if (messageList) {
            if (savedScrollPos !== undefined) {
              messageList.scrollTop = savedScrollPos;
            } else {
              messageList.scrollTop = messageList.scrollHeight;
            }
          }
        });
        if (galleryView && !galleryView.classList.contains("hidden")) {
          void loadGallery(true);
        }
      } else {
        captureViewportAnchor();
        const prevHeight = messageList.scrollHeight;
        messageStore = asc.concat(messageStore);
        asc.forEach((msg) => messageById.set(msg.id, msg));
        renderMessages(asc, "prepend");
        void fetchReactionsForMessages(asc.map((msg) => msg.id));
        void fetchTagsForMessages(asc.map((msg) => msg.id));
        const delta = messageList.scrollHeight - prevHeight;
        messageList.scrollTop = messageList.scrollTop + delta;
      }
    }
  } finally {
    isLoadingMessages = false;
  }
}

async function loadNewerMessages() {
  if (!isTauri || !messageList || !currentThreadId || !currentAfterTs) return;
  if (isLoadingMessages) return;
  isLoadingMessages = true;
  const requestId = ++messagesRequestId;
  try {
    const messages: MessageRow[] = await apiListMessagesAfter(
      currentThreadId,
      currentAfterTs,
      currentAfterId,
      PAGE_SIZE,
    );
    if (requestId !== messagesRequestId) return;
    if (messages.length > 0) {
      const asc = messages;
      const newest = asc[asc.length - 1];
      currentAfterTs = messageSortTs(newest);
      currentAfterId = newest.id;
      messageStore = messageStore.concat(asc);
      asc.forEach((msg) => messageById.set(msg.id, msg));
      renderMessages(asc, "append");
      void fetchReactionsForMessages(asc.map((msg) => msg.id));
      void fetchTagsForMessages(asc.map((msg) => msg.id));
    }
  } finally {
    isLoadingMessages = false;
  }
}

importBtn?.addEventListener("click", () => {
  if (importPanel) {
    importPanel.classList.toggle("hidden");
  }
  // Auto-close menu when opening import panel
  if (optionsMenu && !importPanel?.classList.contains("hidden")) {
    optionsMenu.classList.add("hidden");
  }
});

document.getElementById("close-import-panel")?.addEventListener("click", () => {
  importPanel?.classList.add("hidden");
});

chooseFileBtn?.addEventListener("click", async () => {
  if (!isTauri) {
    if (statusEl) statusEl.textContent = "File picker only available in Tauri.";
    return;
  }
  try {
    const result = await open({
      multiple: false,
      filters: [{ name: "Signal Backup", extensions: ["backup"] }],
    });
    if (typeof result === "string") {
      selectedBackupPath = result;
      if (importFileEl) importFileEl.textContent = result;
    }
  } catch (err) {
    if (statusEl) statusEl.textContent = `File picker failed: ${err}`;
  }
});

runImportBtn?.addEventListener("click", async () => {
  if (!isTauri) {
    if (statusEl) statusEl.textContent = "Import only available in Tauri.";
    return;
  }
  if (isBusy) return;
  if (!selectedBackupPath) {
    if (statusEl) statusEl.textContent = "Choose a .backup file first.";
    return;
  }
  const passphrase = passphraseInput?.value ?? "";
  if (!passphrase.trim()) {
    if (statusEl) statusEl.textContent = "Enter the 30-digit passphrase.";
    return;
  }
  try {
    setBusy(true, "Importing... (this may take a few minutes)");
    await apiImportBackup(selectedBackupPath, passphrase);
    if (passphraseInput) passphraseInput.value = "";
    if (passphraseCount) passphraseCount.textContent = "0/30";
    await refreshThreads();
    if (statusEl) statusEl.textContent = "Import complete.";
  } catch (err) {
    if (statusEl) statusEl.textContent = `Import failed: ${err}`;
  } finally {
    setBusy(false);
  }
});

passphraseInput?.addEventListener("input", () => {
  const raw = passphraseInput.value ?? "";
  const normalized = raw.replace(/[-\\s]/g, "");
  if (passphraseCount) passphraseCount.textContent = `${normalized.length}/30`;
});

passphraseInput?.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    runImportBtn?.click();
  }
});

seedBtn?.addEventListener("click", async () => {
  if (!isTauri) {
    if (statusEl) statusEl.textContent = "Seed unavailable outside Tauri.";
    return;
  }
  if (isBusy) return;
  if (statusEl) statusEl.textContent = "Seeding demo data...";
  try {
    await apiSeedDemo(50000, 49);
    await refreshThreads();
    if (statusEl) statusEl.textContent = "Demo data loaded (50k + 49).";
  } catch (err) {
    if (statusEl) statusEl.textContent = `Seed failed: ${err}`;
  }
});

resetBtn?.addEventListener("click", async () => {
  if (!isTauri) {
    if (statusEl) statusEl.textContent = "Reset unavailable outside Tauri.";
    return;
  }
  if (isBusy) return;

  // First click: enter confirmation state
  if (resetConfirmState === "initial") {
    resetConfirmState = "confirming";
    resetBtn.setAttribute("data-confirm-state", "confirming");
    resetBtn.textContent = "Click again to confirm";

    // Auto-revert after 3 seconds
    if (resetConfirmTimeout) clearTimeout(resetConfirmTimeout);
    resetConfirmTimeout = window.setTimeout(() => {
      resetConfirmState = "initial";
      resetBtn.setAttribute("data-confirm-state", "initial");
      resetBtn.textContent = "Reset archive";
    }, 3000);

    return;
  }

  // Second click: execute reset
  if (resetConfirmTimeout) clearTimeout(resetConfirmTimeout);
  resetConfirmState = "initial";
  resetBtn.setAttribute("data-confirm-state", "initial");
  resetBtn.textContent = "Reset archive";

  if (statusEl) statusEl.textContent = "Resetting archive...";
  try {
    await apiResetArchive();
    await refreshThreads();
    resetArchiveState();
    toggleTab("messages");
    if (statusEl) statusEl.textContent = "Archive reset.";
  } catch (err) {
    if (statusEl) statusEl.textContent = `Reset failed: ${err}`;
  }
});

copyDiagBtn?.addEventListener("click", async () => {
  if (!isTauri) {
    if (statusEl) statusEl.textContent = "Diagnostics unavailable outside Tauri.";
    return;
  }
  try {
    const content: string = await apiGetDiagnostics();
    await writeText(content);
    if (statusEl) statusEl.textContent = "Diagnostics copied.";
  } catch (err) {
    if (statusEl) statusEl.textContent = `Diagnostics failed: ${err}`;
  }
});

const onMessageScroll = throttleRaf(() => {
  if (!messageList || !currentThreadId || isLoadingMessages) return;
  scheduleAnchorCapture();
  const scrollTop = messageList.scrollTop;
  const scrollHeight = messageList.scrollHeight;
  const clientHeight = messageList.clientHeight;
  if (scrollTop <= 40) {
    void loadMessages(currentThreadId, false);
  }
  if (scrollTop + clientHeight >= scrollHeight - 40) {
    void loadNewerMessages();
  }
});
messageList?.addEventListener("scroll", onMessageScroll);

messageList?.addEventListener("click", (event) => {
  const target = event.target as HTMLElement | null;
  if (!target) return;
  const tagTrigger = target.closest(".message-tag-trigger") as HTMLElement | null;
  if (tagTrigger) {
    event.stopPropagation();
    const messageId = tagTrigger.dataset.messageId;
    if (messageId) {
      void showTagPicker(messageId, tagTrigger);
    }
    return;
  }
  const attachmentEl = target.closest("[data-attachment-id]") as HTMLElement | null;
  if (attachmentEl) {
    const attachmentId = attachmentEl.dataset.attachmentId;
    if (!attachmentId) return;
    const attachment = attachmentsById.get(attachmentId);
    if (attachment) {
      openLightbox(attachment);
    }
  }
});

function setContentPane(mode: "messages" | "search" | "gallery" | "scrapbook") {
  if (!messagesView || !searchView || !galleryView || !scrapbookView || !tabMessages || !tabGallery || !tabScrapbook) return;
  messagesView.classList.toggle("hidden", mode !== "messages");
  searchView.classList.toggle("hidden", mode !== "search");
  galleryView.classList.toggle("hidden", mode !== "gallery");
  scrapbookView.classList.toggle("hidden", mode !== "scrapbook");
  tabMessages.classList.toggle("active", mode === "messages" || mode === "search");
  tabGallery.classList.toggle("active", mode === "gallery");
  tabScrapbook.classList.toggle("active", mode === "scrapbook");
}

function clearSearchState() {
  if (searchInput) {
    searchInput.value = "";
  }
  searchHits = [];
  searchIndex = -1;
  searchQuery = "";
  searchMatchIds = new Set();
  highlightMessageId = null;
  updateSearchControls();
  setContentPane("messages");
}

function toggleTab(tab: "messages" | "gallery" | "scrapbook") {
  if (tab === "scrapbook") {
    setContentPane("scrapbook");
    if (currentScrapbookTagId) void loadScrapbook(true);
  } else if (tab === "gallery") {
    setContentPane("gallery");
    void loadGallery(true);
  } else {
    setContentPane("messages");
  }
}

async function loadGallery(reset: boolean) {
  if (!isTauri || !galleryGrid || !currentThreadId || galleryLoading) return;
  galleryLoading = true;
  const requestId = ++galleryRequestId;
  try {
    if (reset) {
      galleryOffset = 0;
      galleryHasMore = true;
    galleryGrid.replaceChildren();
      galleryItems = [];
    }
    if (!galleryHasMore) return;
    const fromTs = parseDateFromInput(galleryFrom, false);
    const toTs = parseDateFromInput(galleryTo, true);
    const bucket = sizeBucket(gallerySize?.value ?? "all");
    const sort = gallerySort?.value ?? "date_desc";
    const items: ThreadMediaRow[] = await apiListThreadMedia(
      currentThreadId,
      fromTs,
      toTs,
      bucket,
      sort,
      GALLERY_PAGE,
      galleryOffset,
    );
    if (requestId !== galleryRequestId) return;
    if (items.length < GALLERY_PAGE) {
      galleryHasMore = false;
    }
    galleryOffset += items.length;
    galleryItems = galleryItems.concat(items);
    renderGalleryItems(items);
  } finally {
    galleryLoading = false;
  }
}

function renderGalleryItems(items: ThreadMediaRow[]) {
  if (!galleryGrid) return;
  const fragment = document.createDocumentFragment();
  items.forEach((item) => {
    const card = document.createElement("div");
    card.className = "gallery-item";
    const meta = document.createElement("div");
    meta.className = "meta";
    const ts = mediaSortTs(item);
    const tsLabel = ts ? new Date(ts).toLocaleString() : "";
    meta.textContent = `${item.original_filename ?? item.mime ?? "Media"}${tsLabel ? ` · ${tsLabel}` : ""}`;
    const sizeBytes = item.size_bytes ?? 0;
    const forceClick = item.kind === "video" || item.kind === "audio" || sizeBytes >= 8 * 1024 * 1024;
    if (requireMediaClick || forceClick) {
      insertGalleryPlaceholder(card, item, meta, forceClick);
    } else {
      renderGalleryMedia(card, item);
    }
    card.appendChild(meta);
    fragment.appendChild(card);
  });
  galleryGrid.appendChild(fragment);
}

function insertGalleryPlaceholder(
  card: HTMLDivElement,
  item: ThreadMediaRow,
  meta: HTMLDivElement,
  forceClick: boolean,
) {
  const label = requireMediaClick ? "Media hidden" : "Large media — click to load";
  const placeholder = createMediaPlaceholder("gallery", label);
  attachPlaceholderClick(placeholder, () => {
    placeholder.remove();
    renderGalleryMedia(card, item);
    if (requireMediaClick) {
      addHideMediaButton(card, () => {
        card.querySelectorAll("img, video, audio").forEach((el) => el.remove());
        card.querySelectorAll(".media-hide-btn").forEach((el) => el.remove());
        insertGalleryPlaceholder(card, item, meta, forceClick);
      }, meta);
    }
  });
  if (meta.parentElement === card) {
    card.insertBefore(placeholder, meta);
  } else {
    card.appendChild(placeholder);
  }
}

function toMediaAssetFromAttachment(attachment: AttachmentRow): MediaAsset {
  return {
    id: attachment.id,
    sha256: attachment.sha256,
    mime: attachment.mime ?? null,
    size_bytes: attachment.size_bytes ?? null,
    original_filename: attachment.original_filename ?? null,
    kind: attachment.kind ?? null,
    width: attachment.width ?? null,
    height: attachment.height ?? null,
    duration_ms: attachment.duration_ms ?? null,
  };
}

function toMediaAssetFromThreadMedia(item: ThreadMediaRow): MediaAsset {
  return {
    id: item.id,
    sha256: item.sha256,
    mime: item.mime ?? null,
    size_bytes: item.size_bytes ?? null,
    original_filename: item.original_filename ?? null,
    kind: item.kind ?? null,
    width: item.width ?? null,
    height: item.height ?? null,
    duration_ms: item.duration_ms ?? null,
  };
}

function renderGalleryMedia(card: HTMLDivElement, item: ThreadMediaRow) {
  if (!item.kind || !item.mime) return;
  const asset = toMediaAssetFromThreadMedia(item);
  if (item.kind === "image") {
    const img = document.createElement("img");
    img.alt = item.original_filename ?? "image";
    card.prepend(img);
    img.addEventListener("click", () => {
      lightboxGallery = galleryItems.map(toMediaAssetFromThreadMedia);
      lightboxIndex = lightboxGallery.findIndex((entry) => entry.id === item.id);
      openLightbox(asset);
    });
    void applyThumbnailSource(img, asset);
    return;
  }
  if (item.kind === "video") {
    const video = document.createElement("video");
    video.controls = false;
    video.preload = "metadata";
    video.muted = true;
    video.playsInline = true;
    video.classList.add("gallery-video");
    video.addEventListener("mouseenter", () => {
      void video.play();
    });
    video.addEventListener("mouseleave", () => {
      video.pause();
      try {
        video.currentTime = 0;
      } catch {
        // ignore reset failures
      }
    });
    card.prepend(video);
    video.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      video.pause();
      try {
        video.currentTime = 0;
      } catch {
        // ignore reset failures
      }
      lightboxGallery = galleryItems.map(toMediaAssetFromThreadMedia);
      lightboxIndex = lightboxGallery.findIndex((entry) => entry.id === item.id);
      openLightbox(asset);
    });
    void applyMediaSource(video, asset);
    return;
  }
  if (item.kind === "audio") {
    const audio = document.createElement("audio");
    audio.controls = true;
    card.prepend(audio);
    audio.addEventListener("click", () => {
      lightboxGallery = galleryItems.map(toMediaAssetFromThreadMedia);
      lightboxIndex = lightboxGallery.findIndex((entry) => entry.id === item.id);
      openLightbox(asset);
    });
    void applyMediaSource(audio, asset);
  }
}

async function loadAttachments(messageId: string, container: HTMLElement, mode: "button" | "auto") {
  if (!isTauri) return;
  if (!container) return;
  if (attachmentCache.has(messageId)) {
    const cached = attachmentCache.get(messageId) || [];
    if (!cached.length) return;
    // Ensure messageAttachmentsCache is populated even from cache
    if (!messageAttachmentsCache.has(messageId)) {
      const assets = cached.map(toMediaAssetFromAttachment);
      messageAttachmentsCache.set(messageId, assets);
    }
    if (mode === "button") {
      renderAttachmentButton(container, messageId);
    } else {
      renderAttachments(container, cached, messageId);
    }
    return;
  }
  try {
    const attachments: AttachmentRow[] = await apiListMessageAttachments(messageId);
    attachmentCache.set(messageId, attachments);
    const assets = attachments.map(toMediaAssetFromAttachment);
    assets.forEach((asset) => attachmentsById.set(asset.id, asset));
    messageAttachmentsCache.set(messageId, assets);
    if (!attachments.length) return;
    if (mode === "button") {
      renderAttachmentButton(container, messageId);
    } else {
      renderAttachments(container, attachments, messageId);
    }
  } catch {
    // ignore
  }
}

async function loadThreadMedia(threadId: string) {
  if (!isTauri) return;
  if (threadMediaCache.has(threadId)) return;

  try {
    // Fetch all media from thread sorted by date (newest first to match message order)
    const items: ThreadMediaRow[] = await apiListThreadMedia(
      threadId,
      null,  // no date filter
      null,
      null,  // all sizes
      "date_desc",  // newest first
      10000,  // high limit to get all media
      0
    );

    const assets = items.map(toMediaAssetFromThreadMedia);
    threadMediaCache.set(threadId, assets);
  } catch {
    // ignore errors, cache will remain empty
  }
}

async function loadAttachmentDataUrl(attachment: MediaAsset): Promise<string | null> {
  const key = attachment.sha256;
  if (attachmentDataCache.has(key)) {
    return attachmentDataCache.get(key) || null;
  }
  const mime = attachment.mime ?? "";
  try {
    const dataUrl: string = await attachmentDataUrl(key, mime);
    attachmentDataCache.set(key, dataUrl);
    return dataUrl;
  } catch {
    return null;
  }
}

async function loadAttachmentFileUrl(attachment: MediaAsset): Promise<string | null> {
  const key = attachment.sha256;
  if (attachmentFileCache.has(key)) {
    return attachmentFileCache.get(key) || null;
  }
  try {
    const path: string = await attachmentPath(key, attachment.mime ?? null);
    const src = convertFileSrc(path);
    attachmentFileCache.set(key, src);
    return src;
  } catch {
    return null;
  }
}

async function loadAttachmentThumbUrl(attachment: MediaAsset, maxSize: number): Promise<string | null> {
  const key = `${attachment.sha256}:${maxSize}`;
  if (attachmentThumbCache.has(key)) {
    return attachmentThumbCache.get(key) || null;
  }
  try {
    const path: string = await attachmentThumbnail(attachment.sha256, attachment.mime ?? null, maxSize);
    const src = convertFileSrc(path);
    attachmentThumbCache.set(key, src);
    return src;
  } catch {
    return null;
  }
}

async function loadAttachmentSrcInfo(attachment: MediaAsset): Promise<SourceInfo | null> {
  if (attachment.kind === "video" || attachment.kind === "audio") {
    const src = await loadAttachmentFileUrl(attachment);
    return src ? { src, via: "file" } : null;
  }
  const size = attachment.size_bytes ?? 0;
  if (size > 8 * 1024 * 1024) {
    const src = await loadAttachmentFileUrl(attachment);
    return src ? { src, via: "file" } : null;
  }
  const dataUrl = await loadAttachmentDataUrl(attachment);
  if (dataUrl) {
    return { src: dataUrl, via: "data" };
  }
  const fallback = await loadAttachmentFileUrl(attachment);
  return fallback ? { src: fallback, via: "file" } : null;
}

async function applyThumbnailSource(img: HTMLImageElement, attachment: MediaAsset) {
  const thumb = await loadAttachmentThumbUrl(attachment, 320);
  if (thumb) {
    img.src = thumb;
    return;
  }
  await applyMediaSource(img, attachment);
}

async function applyMediaSource(
  element: HTMLImageElement | HTMLVideoElement | HTMLAudioElement,
  attachment: MediaAsset,
) {
  const info = await loadAttachmentSrcInfo(attachment);
  if (!info) {
    element.replaceWith(document.createTextNode("Media preview unavailable"));
    return;
  }
  const mime = attachment.mime ?? undefined;
  const onError = async () => {
    element.removeEventListener("error", onError);
    const fallback =
      info.via === "file" ? await loadAttachmentDataUrl(attachment) : await loadAttachmentFileUrl(attachment);
    if (fallback && fallback !== info.src) {
      setMediaSource(element, fallback, mime);
    } else {
      element.replaceWith(document.createTextNode("Media preview unavailable"));
    }
  };
  element.addEventListener("error", onError, { once: true });
  setMediaSource(element, info.src, mime);
}

let lightboxEl: HTMLDivElement | null = null;
let lightboxContent: HTMLDivElement | null = null;
let lightboxClose: HTMLButtonElement | null = null;

function ensureLightbox() {
  if (lightboxEl && lightboxContent && lightboxClose) return;
  lightboxEl = document.createElement("div");
  lightboxEl.className = "lightbox hidden";
  lightboxContent = document.createElement("div");
  lightboxContent.className = "lightbox-content";
  lightboxClose = document.createElement("button");
  lightboxClose.className = "lightbox-close";
  lightboxClose.textContent = "Close";
  lightboxEl.appendChild(lightboxClose);
  lightboxEl.appendChild(lightboxContent);
  document.body.appendChild(lightboxEl);
  lightboxClose.addEventListener("click", () => void closeLightbox());
  lightboxEl.addEventListener("click", (event) => {
    if (event.target === lightboxEl) {
      void closeLightbox();
    }
  });
  document.addEventListener("keydown", (event) => {
    if (!lightboxEl || lightboxEl.classList.contains("hidden")) return;

    if (event.key === "Escape") {
      void closeLightbox();
      return;
    }

    if (event.key === "ArrowLeft") {
      event.preventDefault();
      navigateLightbox(-1);
      return;
    }

    if (event.key === "ArrowRight") {
      event.preventDefault();
      navigateLightbox(1);
      return;
    }

    if (event.key === "f" || event.key === "F") {
      event.preventDefault();
      toggleLightboxFullscreen();
      return;
    }

    const video = lightboxContent?.querySelector("video");
    if (video) {
      if (event.key === " " || event.key.toLowerCase() === "k") {
        event.preventDefault();
        if (video.paused) {
          void video.play();
        } else {
          video.pause();
        }
        return;
      }

      if (event.key.toLowerCase() === "m") {
        event.preventDefault();
        video.muted = !video.muted;
        return;
      }
    }
  });
}

async function closeLightbox() {
  if (!lightboxEl || !lightboxContent) return;

  // Exit fullscreen if currently in fullscreen mode
  if (isLightboxFullscreen && isTauri) {
    try {
      await getCurrentWindow().setFullscreen(false);
    } catch (error) {
      console.error("Failed to exit fullscreen on close:", error);
    }
  }

  lightboxContent.innerHTML = "";
  lightboxEl.classList.add("hidden");
  lightboxEl.classList.remove("lightbox-fullscreen");
  isLightboxFullscreen = false;
  lightboxGallery = [];
  lightboxIndex = -1;
}

function navigateLightbox(direction: number) {
  if (lightboxGallery.length === 0) return;
  const newIndex = lightboxIndex + direction;
  if (newIndex < 0 || newIndex >= lightboxGallery.length) return;
  lightboxIndex = newIndex;
  openLightbox(lightboxGallery[lightboxIndex]);
}

async function toggleLightboxFullscreen() {
  if (!lightboxEl) return;

  isLightboxFullscreen = !isLightboxFullscreen;

  if (isLightboxFullscreen) {
    lightboxEl.classList.add("lightbox-fullscreen");
    if (isTauri) {
      try {
        await getCurrentWindow().setFullscreen(true);
      } catch (error) {
        console.error("Failed to enter fullscreen:", error);
      }
    }
  } else {
    lightboxEl.classList.remove("lightbox-fullscreen");
    if (isTauri) {
      try {
        await getCurrentWindow().setFullscreen(false);
      } catch (error) {
        console.error("Failed to exit fullscreen:", error);
      }
    }
  }
}

function openLightbox(attachment: MediaAsset) {
  ensureLightbox();
  if (!lightboxEl || !lightboxContent) return;
  lightboxContent.innerHTML = "";
  const title = document.createElement("div");
  title.className = "lightbox-title";
  title.textContent = attachment.original_filename ?? attachment.mime ?? "Media";
  lightboxContent.appendChild(title);
  if (attachment.kind === "image") {
    const img = document.createElement("img");
    img.alt = attachment.original_filename ?? "image";
    lightboxContent.appendChild(img);
    void applyMediaSource(img, attachment);
  } else if (attachment.kind === "video") {
    const video = document.createElement("video");
    video.controls = true;
    video.preload = "metadata";
    lightboxContent.appendChild(video);
    void applyMediaSource(video, attachment);
  } else if (attachment.kind === "audio") {
    const audio = document.createElement("audio");
    audio.controls = true;
    lightboxContent.appendChild(audio);
    void applyMediaSource(audio, attachment);
  }
  lightboxEl.classList.remove("hidden");
}

function renderAttachments(container: HTMLElement, attachments: AttachmentRow[], messageId?: string) {
  if (!attachments.length) {
    container.innerHTML = "";
    return;
  }
  container.innerHTML = "";
  // Use thread-level media cache for navigation
  const threadMedia = currentThreadId ? threadMediaCache.get(currentThreadId) || [] : [];

  attachments.forEach((att) => {
    attachmentsById.set(att.id, toMediaAssetFromAttachment(att));
    const item = document.createElement("div");
    item.className = "media-item";
    const asset = toMediaAssetFromAttachment(att);

    if (att.kind === "image" && att.mime) {
      const img = document.createElement("img");
      img.alt = att.original_filename ?? "image";
      img.dataset.attachmentId = att.id;
      img.classList.add("media-clickable");
      img.addEventListener("click", () => {
        lightboxGallery = threadMedia;
        lightboxIndex = threadMedia.findIndex(a => a.id === asset.id);
        openLightbox(asset);
      });
      item.appendChild(img);
      const meta = document.createElement("div");
      meta.className = "meta";
      meta.textContent = att.original_filename ?? att.mime ?? "image";
      item.appendChild(meta);
      void applyThumbnailSource(img, att);
    } else if (att.kind === "video" && att.mime) {
      const video = document.createElement("video");
      video.controls = true;
      video.preload = "metadata";
      video.dataset.attachmentId = att.id;
      video.classList.add("media-clickable");
      video.addEventListener("click", (e) => {
        // Only open lightbox if clicking on video itself, not controls
        if ((e.target as HTMLElement).tagName === "VIDEO") {
          lightboxGallery = threadMedia;
          lightboxIndex = threadMedia.findIndex(a => a.id === asset.id);
          openLightbox(asset);
        }
      });
      item.appendChild(video);
      const meta = document.createElement("div");
      meta.className = "meta";
      meta.textContent = att.original_filename ?? att.mime ?? "video";
      item.appendChild(meta);
      void applyMediaSource(video, att);
    } else if (att.kind === "audio" && att.mime) {
      const audio = document.createElement("audio");
      audio.controls = true;
      audio.dataset.attachmentId = att.id;
      audio.classList.add("media-clickable");
      audio.addEventListener("click", (e) => {
        // Only open lightbox if clicking on audio element, not controls
        if ((e.target as HTMLElement).tagName === "AUDIO") {
          lightboxGallery = threadMedia;
          lightboxIndex = threadMedia.findIndex(a => a.id === asset.id);
          openLightbox(asset);
        }
      });
      item.appendChild(audio);
      const meta = document.createElement("div");
      meta.className = "meta";
      meta.textContent = att.original_filename ?? att.mime ?? "audio";
      item.appendChild(meta);
      void applyMediaSource(audio, att);
    } else {
      const label = document.createElement("div");
      label.textContent = att.original_filename ?? att.mime ?? "Attachment";
      const meta = document.createElement("div");
      meta.className = "meta";
      meta.textContent = att.mime ?? "file";
      item.appendChild(label);
      item.appendChild(meta);
    }
    container.appendChild(item);
  });

  if (requireMediaClick && messageId) {
    addHideMediaButton(container, () => {
      container.innerHTML = "";
      renderAttachmentButton(container, messageId);
    });
  }
}

function renderAttachmentButton(container: HTMLElement, messageId: string) {
  const placeholder = createMediaPlaceholder("message", "Media hidden");
  attachPlaceholderClick(placeholder, () => {
    container.innerHTML = "";
    renderAttachments(container, attachmentCache.get(messageId) || [], messageId);
  });
  container.appendChild(placeholder);
}

function attachPlaceholderClick(placeholder: HTMLDivElement, onActivate: () => void) {
  placeholder.classList.add("clickable");
  placeholder.setAttribute("role", "button");
  placeholder.setAttribute("tabindex", "0");
  placeholder.addEventListener("click", onActivate);
  placeholder.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onActivate();
    }
  });
}

function addHideMediaButton(
  container: HTMLElement,
  onHide: () => void,
  beforeEl?: HTMLElement,
) {
  const btn = document.createElement("button");
  btn.className = "secondary media-hide-btn";
  btn.type = "button";
  btn.textContent = "Hide media";
  btn.addEventListener("click", onHide);
  if (beforeEl) {
    container.insertBefore(btn, beforeEl);
  } else {
    container.appendChild(btn);
  }
}

searchInput?.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    searchDebounced();
  }
});

searchInput?.addEventListener("input", () => {
  searchDebounced();
});

searchInput?.addEventListener("search", () => {
  if (searchInput && !searchInput.value.trim()) {
    searchDebounced();
  }
});

searchPrevBtn?.addEventListener("click", async () => {
  if (!searchHits.length) return;
  searchIndex = (searchIndex + 1) % searchHits.length;
  updateSearchControls();
  await jumpToHit(searchHits[searchIndex]);
});

searchNextBtn?.addEventListener("click", async () => {
  if (!searchHits.length) return;
  searchIndex = (searchIndex - 1 + searchHits.length) % searchHits.length;
  updateSearchControls();
  await jumpToHit(searchHits[searchIndex]);
});

searchAllBtn?.addEventListener("click", () => {
  if (!searchHits.length) return;
  renderSearchResultsList(searchHits);
});

threadSearchInput?.addEventListener("input", () => {
  threadFilterReload();
});

jumpBtn?.addEventListener("click", async () => {
  if (!isTauri || !currentThreadId) return;

  if (!jumpDateInput || !jumpDateInput.value) {
    if (statusEl) statusEl.textContent = "Pick a date to jump.";
    return;
  }

  const target = parseDateFromInput(jumpDateInput, false);
  if (!target) {
    if (statusEl) statusEl.textContent = "Invalid date format.";
    return;
  }
  try {
    if (statusEl) statusEl.textContent = "Jumping to date...";
    const messages: MessageRow[] = await apiListMessagesAfter(currentThreadId, target, null, PAGE_SIZE);
    if (!messages.length) {
      if (statusEl) statusEl.textContent = "No messages after that date.";
      return;
    }
    messageStore = messages;
    const oldest = messageStore[0];
    const newest = messageStore[messageStore.length - 1];
    currentBeforeTs = messageSortTs(oldest);
    currentBeforeId = oldest.id;
    currentAfterTs = messageSortTs(newest);
    currentAfterId = newest.id;
    renderMessages(messageStore, "replace");
    reactionMap.clear();
    void fetchReactionsForMessages(messageStore.map((msg) => msg.id));
    if (messageList) {
      messageList.scrollTop = 0;
    }
    if (statusEl) statusEl.textContent = "Jump complete.";
  } catch (err) {
    if (statusEl) statusEl.textContent = `Jump failed: ${err}`;
  }
});

jumpToggle?.addEventListener("click", () => {
  if (!jumpControls || !jumpToggle) return;
  jumpControls.classList.toggle("hidden");
  const isOpen = !jumpControls.classList.contains("hidden");
  jumpToggle.textContent = isOpen ? "Jump ▾" : "Jump";
  if (isOpen && jumpDateInput) {
    jumpDateInput.focus();
  }
});

document.addEventListener("click", (event) => {
  if (!jumpControls || !jumpToggle) return;
  const target = event.target as Node;
  if (target instanceof Element && target.closest(".flatpickr-calendar")) {
    return;
  }
  if (!jumpControls.contains(target) && !jumpToggle.contains(target)) {
    if (!jumpControls.classList.contains("hidden")) {
      jumpControls.classList.add("hidden");
      jumpToggle.textContent = "Jump";
    }
  }
});

optionsBtn?.addEventListener("click", () => {
  optionsMenu?.classList.toggle("hidden");
});

document.addEventListener("click", (event) => {
  if (!optionsMenu || !optionsBtn) return;
  const target = event.target as Node;
  if (!optionsMenu.contains(target) && !optionsBtn.contains(target)) {
    optionsMenu.classList.add("hidden");
  }
});

mediaToggle?.addEventListener("click", () => {
  const currentState = mediaToggle.getAttribute("aria-checked") === "true";
  const newState = !currentState;

  mediaToggle.setAttribute("aria-checked", String(newState));
  requireMediaClick = newState;
  localStorage.setItem("gt_media_click", String(newState));

  if (currentThreadId) {
    attachmentCache.clear();
    attachmentDataCache.clear();
    attachmentFileCache.clear();
    attachmentThumbCache.clear();
    attachmentsById.clear();
    threadMediaCache.clear();
    void loadMessages(currentThreadId, true);
    void loadGallery(true);
  }
});

darkModeToggle?.addEventListener("click", () => {
  const currentState = darkModeToggle.getAttribute("aria-checked") === "true";
  const newState = !currentState;

  darkModeToggle.setAttribute("aria-checked", String(newState));
  document.documentElement.setAttribute("data-theme", newState ? "dark" : "light");
  localStorage.setItem("gt_dark_mode", String(newState));
});

// Color swatch selection
document.querySelectorAll(".color-swatch").forEach(swatch => {
  swatch.addEventListener("click", (event) => {
    const target = event.currentTarget as HTMLElement;
    const selectedColor = target.dataset.color;

    if (!selectedColor) return;

    // Update all swatches
    document.querySelectorAll(".color-swatch").forEach(sw => {
      const isActive = (sw as HTMLElement).dataset.color === selectedColor;
      sw.classList.toggle("active", isActive);
      sw.setAttribute("aria-checked", String(isActive));
    });

    // Apply theme
    document.documentElement.setAttribute("data-accent", selectedColor);
    localStorage.setItem("gt_accent_color", selectedColor);
  });
});

// Keyboard navigation for swatches
document.querySelector(".color-swatch-group")?.addEventListener("keydown", (event: Event) => {
  const keyEvent = event as KeyboardEvent;
  const swatches = Array.from(document.querySelectorAll(".color-swatch")) as HTMLElement[];
  const currentIndex = swatches.findIndex(sw => sw.classList.contains("active"));

  let nextIndex = currentIndex;

  if (keyEvent.key === "ArrowLeft" || keyEvent.key === "ArrowUp") {
    keyEvent.preventDefault();
    nextIndex = currentIndex > 0 ? currentIndex - 1 : swatches.length - 1;
  } else if (keyEvent.key === "ArrowRight" || keyEvent.key === "ArrowDown") {
    keyEvent.preventDefault();
    nextIndex = currentIndex < swatches.length - 1 ? currentIndex + 1 : 0;
  } else if (keyEvent.key === " " || keyEvent.key === "Enter") {
    keyEvent.preventDefault();
    (keyEvent.target as HTMLElement).click();
    return;
  }

  if (nextIndex !== currentIndex) {
    swatches[nextIndex].focus();
  }
});

tabMessages?.addEventListener("click", () => {
  toggleTab("messages");
});

tabGallery?.addEventListener("click", () => {
  toggleTab("gallery");
  void loadGallery(true);
});

tabScrapbook?.addEventListener("click", () => {
  toggleTab("scrapbook");
});

scrapbookTagSelect?.addEventListener("change", () => {
  currentScrapbookTagId = scrapbookTagSelect.value || null;
  if (currentScrapbookTagId) {
    void loadScrapbook(true);
  } else {
    if (scrapbookMessageList) scrapbookMessageList.replaceChildren();
  }
});

const onScrapbookScroll = throttleRaf(() => {
  if (!scrapbookMessageList || !currentScrapbookTagId || isLoadingScrapbook) return;
  if (scrapbookMessageList.scrollTop <= 40) {
    void loadScrapbook(false);
  }
});
scrapbookMessageList?.addEventListener("scroll", onScrapbookScroll);

gallerySize?.addEventListener("change", galleryFilterReload);
gallerySort?.addEventListener("change", galleryFilterReload);

galleryFromClear?.addEventListener("click", () => {
  if (galleryFrom) {
    galleryFrom.value = "";
    // Clear the Flatpickr instance
    const fp = (galleryFrom as any)._flatpickr;
    if (fp) fp.clear();
  }
  galleryFilterReload();
});

galleryToClear?.addEventListener("click", () => {
  if (galleryTo) {
    galleryTo.value = "";
    // Clear the Flatpickr instance
    const fp = (galleryTo as any)._flatpickr;
    if (fp) fp.clear();
  }
  galleryFilterReload();
});

const onGalleryScroll = throttleRaf(() => {
  if (!galleryGrid || galleryLoading || !galleryHasMore) return;
  if (galleryGrid.scrollTop + galleryGrid.clientHeight >= galleryGrid.scrollHeight - 80) {
    void loadGallery(false);
  }
});
galleryGrid?.addEventListener("scroll", onGalleryScroll);

updateSearchControls();

// Initialize Flatpickr date pickers
if (jumpDateInput) {
  flatpickr(jumpDateInput, {
    dateFormat: "Y-m-d",
    allowInput: false,
    clickOpens: true,
  });
}

if (galleryFrom) {
  flatpickr(galleryFrom, {
    dateFormat: "Y-m-d",
    allowInput: false,
    clickOpens: true,
    onChange: () => galleryFilterReload(),
  });
}

if (galleryTo) {
  flatpickr(galleryTo, {
    dateFormat: "Y-m-d",
    allowInput: false,
    clickOpens: true,
    onChange: () => galleryFilterReload(),
  });
}

function tagColorClass(color: string) {
  const index = TAG_COLOR_PRESETS.findIndex((preset) => preset.toLowerCase() === color.toLowerCase());
  if (index >= 0) {
    return `tag-color-${index + 1}`;
  }
  return "tag-color-default";
}

function setTagColorPreset(color: string) {
  selectedTagColor = color;
  applyCreateTagButtonColor(color);
  if (!tagColorPresets) return;
  tagColorPresets.querySelectorAll(".tag-color-preset").forEach((button) => {
    const isActive = (button as HTMLButtonElement).dataset.color === color;
    button.classList.toggle("active", isActive);
    button.setAttribute("aria-pressed", String(isActive));
  });
}

function parseHexColor(color: string) {
  const cleaned = color.replace("#", "").trim();
  if (cleaned.length !== 6) return null;
  const r = Number.parseInt(cleaned.slice(0, 2), 16);
  const g = Number.parseInt(cleaned.slice(2, 4), 16);
  const b = Number.parseInt(cleaned.slice(4, 6), 16);
  if (Number.isNaN(r) || Number.isNaN(g) || Number.isNaN(b)) return null;
  return { r, g, b };
}

function tagButtonTextColor(color: string) {
  const rgb = parseHexColor(color);
  if (!rgb) return "var(--color-white)";
  const luminance = (0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b) / 255;
  return luminance > 0.6 ? "#1b1b1b" : "var(--color-white)";
}

function applyCreateTagButtonColor(color: string) {
  if (!createTagBtn) return;
  const rgb = parseHexColor(color);
  const focus = rgb ? `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.35)` : "var(--color-primary-light)";
  createTagBtn.style.setProperty("--tag-create-color", color);
  createTagBtn.style.setProperty("--tag-create-focus", focus);
  createTagBtn.style.backgroundColor = color;
  createTagBtn.style.borderColor = color;
  createTagBtn.style.color = tagButtonTextColor(color);
}

function initTagColorPresets() {
  if (!tagColorPresets) return;
  tagColorPresets.replaceChildren();
  TAG_COLOR_PRESETS.forEach((color) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `tag-color-preset ${tagColorClass(color)}`;
    button.dataset.color = color;
    button.setAttribute("aria-pressed", "false");
    button.setAttribute("aria-label", `Tag color ${color}`);
    button.addEventListener("click", () => setTagColorPreset(color));
    tagColorPresets.appendChild(button);
  });
  const initial = TAG_COLOR_PRESETS[0];
  setTagColorPreset(initial);
}

// Tag management functions
async function refreshTags() {
  if (!isTauri) return;
  tagsStore = await apiListTags();
  renderTagSelector();
  renderTagManager();
}

function renderTagSelector() {
  if (!scrapbookTagSelect) return;
  const current = scrapbookTagSelect.value;
  const placeholder = document.createElement("option");
  placeholder.value = "";
  placeholder.textContent = "Select a tag...";
  scrapbookTagSelect.replaceChildren(placeholder);
  tagsStore.forEach((tag) => {
    const opt = document.createElement("option");
    opt.value = tag.id;
    opt.textContent = tag.name;
    scrapbookTagSelect.appendChild(opt);
  });
  if (current) scrapbookTagSelect.value = current;
}

function renderTagManager() {
  if (!tagList) return;
  resetDeleteTagConfirm();
  tagList.replaceChildren();
  tagsStore.forEach((tag) => {
    const item = document.createElement("div");
    item.className = "tag-item";

    const color = document.createElement("span");
    color.className = "tag-item-color";
    color.className = `tag-item-color ${tagColorClass(tag.color)}`;

    const name = document.createElement("span");
    name.className = "tag-item-name";
    name.textContent = tag.name;

    const deleteBtn = document.createElement("button");
    deleteBtn.className = "secondary tag-delete-btn";
    deleteBtn.type = "button";
    deleteBtn.textContent = "Delete";
    deleteBtn.addEventListener("click", async (e) => {
      e.preventDefault();
      e.stopPropagation();
      handleDeleteTagClick(tag.id, deleteBtn);
    });

    item.append(color, name, deleteBtn);
    tagList.appendChild(item);
  });
}

async function createTag(name: string, color: string) {
  if (!isTauri) return;
  try {
    await apiCreateTag(name, color);
    await refreshTags();
    if (statusEl) statusEl.textContent = `Tag "${name}" created.`;
  } catch (err) {
    if (statusEl) statusEl.textContent = `Error: ${err}`;
  }
}

async function deleteTag(tagId: string) {
  try {
    await apiDeleteTag(tagId);
    messageTagsCache.clear();
    await refreshTags();
    // Re-render all visible message tags
    refreshVisibleMessageTags();
    if (statusEl) statusEl.textContent = "Tag deleted.";
  } catch (err) {
    if (statusEl) statusEl.textContent = `Error: ${err}`;
  }
}

function resetDeleteTagConfirm() {
  if (deleteTagConfirmTimeout) {
    clearTimeout(deleteTagConfirmTimeout);
    deleteTagConfirmTimeout = null;
  }
  deleteTagConfirmId = null;
  if (!tagList) return;
  tagList.querySelectorAll<HTMLButtonElement>(".tag-delete-btn").forEach((button) => {
    button.textContent = "Delete";
    button.setAttribute("data-confirm-state", "initial");
  });
}

function handleDeleteTagClick(tagId: string, button: HTMLButtonElement) {
  if (deleteTagConfirmId !== tagId) {
    resetDeleteTagConfirm();
    deleteTagConfirmId = tagId;
    button.textContent = "Click again to delete";
    button.setAttribute("data-confirm-state", "confirming");
    deleteTagConfirmTimeout = window.setTimeout(() => {
      resetDeleteTagConfirm();
    }, 3000);
    return;
  }

  resetDeleteTagConfirm();
  void deleteTag(tagId);
}

function refreshVisibleMessageTags() {
  // Clear all tag dots from visible messages
  document.querySelectorAll(".tag-dots").forEach((container) => {
    container.replaceChildren();
  });
  // Re-fetch tags for all visible messages
  const visibleMessageIds = messageStore.map((m) => m.id);
  if (visibleMessageIds.length > 0) {
    void fetchTagsForMessages(visibleMessageIds);
  }
}

async function fetchTagsForMessages(messageIds: string[]) {
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

function renderMessageTags(messageId: string, tags: Tag[]) {
  const container = document.querySelector(`.tag-dots[data-message-id="${messageId}"]`);
  if (!container) return;
  const fragment = document.createDocumentFragment();
  tags.forEach((tag) => {
    const dot = document.createElement("span");
    dot.className = "tag-dot";
    dot.className = `tag-dot ${tagColorClass(tag.color)}`;
    dot.title = tag.name;
    fragment.appendChild(dot);
  });
  container.replaceChildren(fragment);
}

function positionTagPicker(picker: HTMLDivElement, anchor: HTMLElement) {
  const gap = 8;
  const pad = 8;
  picker.style.position = "fixed";
  picker.style.left = "0px";
  picker.style.top = "0px";
  picker.style.right = "auto";
  picker.style.bottom = "auto";

  const anchorRect = anchor.getBoundingClientRect();
  const pickerRect = picker.getBoundingClientRect();
  const viewportW = window.innerWidth;
  const viewportH = window.innerHeight;

  const candidates = [
    { left: anchorRect.left, top: anchorRect.bottom + gap }, // bottom-left
    { left: anchorRect.right - pickerRect.width, top: anchorRect.bottom + gap }, // bottom-right
    { left: anchorRect.left, top: anchorRect.top - pickerRect.height - gap }, // top-left
    { left: anchorRect.right - pickerRect.width, top: anchorRect.top - pickerRect.height - gap }, // top-right
  ];

  let chosen = candidates.find((pos) => {
    const right = pos.left + pickerRect.width;
    const bottom = pos.top + pickerRect.height;
    return pos.left >= pad && pos.top >= pad && right <= viewportW - pad && bottom <= viewportH - pad;
  });

  if (!chosen) {
    const fallback = candidates[0];
    const clampedLeft = Math.min(
      Math.max(fallback.left, pad),
      viewportW - pickerRect.width - pad,
    );
    const clampedTop = Math.min(
      Math.max(fallback.top, pad),
      viewportH - pickerRect.height - pad,
    );
    chosen = { left: clampedLeft, top: clampedTop };
  }

  picker.style.left = `${chosen.left}px`;
  picker.style.top = `${chosen.top}px`;
}

async function showTagPicker(messageId: string, triggerEl: HTMLElement) {
  if (!isTauri) return;

  let picker = document.getElementById("tag-picker") as HTMLDivElement | null;
  if (!picker) {
    picker = document.createElement("div");
    picker.id = "tag-picker";
    picker.className = "tag-picker";
  }
  const anchor = triggerEl.parentElement;
  if (anchor && picker.parentElement !== anchor) {
    anchor.appendChild(picker);
  }

  const currentTags = await apiGetMessageTags(messageId);
  const selectedIds = new Set(currentTags.map((t) => t.id));

  if (tagsStore.length === 0) {
    const empty = document.createElement("div");
    empty.textContent = "No tags yet. Create one in Scrapbook.";
    picker.replaceChildren(empty);
  } else {
    picker.replaceChildren();
    tagsStore.forEach((tag) => {
      const item = document.createElement("div");
      const isSelected = selectedIds.has(tag.id);
      item.className = "tag-picker-item";
      if (isSelected) {
        item.classList.add("selected");
      }

      const color = document.createElement("span");
      color.className = "tag-picker-color";
      color.className = `tag-picker-color ${tagColorClass(tag.color)}`;

      const name = document.createElement("span");
      name.textContent = tag.name;

      const checkmark = document.createElement("span");
      checkmark.className = "tag-picker-checkmark";
      checkmark.textContent = "✓";
      checkmark.classList.toggle("visible", isSelected);

      item.addEventListener("click", () => {
        void toggleMessageTag(messageId, tag.id, !isSelected);
        // Close and reopen picker to update selection state
        picker!.classList.add("hidden");
        setTimeout(() => void showTagPicker(messageId, triggerEl), 50);
      });

      item.append(color, name, checkmark);
      picker!.appendChild(item);
    });
  }

  picker.classList.remove("hidden");
  requestAnimationFrame(() => positionTagPicker(picker!, triggerEl));

  const closeHandler = (e: MouseEvent) => {
    if (!picker?.contains(e.target as Node) && e.target !== triggerEl) {
      picker.classList.add("hidden");
      document.removeEventListener("click", closeHandler);
    }
  };
  setTimeout(() => document.addEventListener("click", closeHandler), 100);
}

async function toggleMessageTag(messageId: string, tagId: string, add: boolean) {
  if (!isTauri) return;
  try {
    const current = await invoke<Tag[]>("get_message_tags_cmd", { messageId });
    const currentIds = current.map((t) => t.id);
    const newIds = add ? [...currentIds, tagId] : currentIds.filter((id) => id !== tagId);
    await apiSetMessageTags(messageId, newIds);

    const updated = tagsStore.filter((t) => newIds.includes(t.id));
    messageTagsCache.set(messageId, updated);
    renderMessageTags(messageId, updated);
  } catch (err) {
    console.error("Tag toggle failed:", err);
  }
}

async function loadScrapbook(reset: boolean) {
  if (!isTauri || !currentScrapbookTagId || isLoadingScrapbook || !scrapbookMessageList) return;

  isLoadingScrapbook = true;
  const requestId = ++scrapbookRequestId;
  try {
    if (reset) {
      if (currentScrapbookTagId) {
        scrapbookScrollPositions.set(currentScrapbookTagId, scrapbookMessageList.scrollTop);
      }
      scrapbookBeforeTs = null;
      scrapbookBeforeId = null;
      scrapbookMessages = [];
      scrapbookMessageList.replaceChildren();
    }

    const msgs = await apiListScrapbookMessages(
      currentScrapbookTagId,
      scrapbookBeforeTs,
      scrapbookBeforeId,
      PAGE_SIZE,
    );
    if (requestId !== scrapbookRequestId) return;

    if (msgs.length > 0) {
      const oldest = msgs[0].message;
      scrapbookBeforeTs = messageSortTs(oldest);
      scrapbookBeforeId = oldest.id;
      scrapbookMessages.push(...msgs);
      renderScrapbookMessages(msgs, reset ? "replace" : "prepend");

      if (reset) {
        const savedPos = scrapbookScrollPositions.get(currentScrapbookTagId);
        requestAnimationFrame(() => {
          if (scrapbookMessageList) {
            scrapbookMessageList.scrollTop = savedPos ?? scrapbookMessageList.scrollHeight;
          }
        });
      }
    }
  } finally {
    isLoadingScrapbook = false;
  }
}

function renderScrapbookMessages(msgs: ScrapbookMessage[], mode: "replace" | "prepend") {
  if (!scrapbookMessageList) return;
  if (mode === "replace") scrapbookMessageList.replaceChildren();

  const frag = document.createDocumentFragment();
  const existingFirstMessage = mode === "prepend"
    ? (scrapbookMessageList.querySelector(".message") as HTMLDivElement | null)
    : null;
  const existingFirstThreadId = existingFirstMessage?.dataset.threadId ?? null;
  let previousThreadId: string | null = null;
  let lastThreadIdInFragment: string | null = null;

  const appendSeparator = () => {
    const disc = document.createElement("div");
    disc.className = "message-discontinuity";
    disc.textContent = "⋯";
    frag.appendChild(disc);
  };

  msgs.forEach((item) => {
    const threadChanged = previousThreadId !== null && previousThreadId !== item.message.thread_id;
    if (item.is_discontinuous || threadChanged) {
      appendSeparator();
    }

    const div = document.createElement("div");
    div.className = `message ${item.message.is_outgoing ? "outgoing" : ""}`;
    div.classList.add("scrapbook-message");
    div.dataset.messageId = item.message.id;
    div.dataset.threadId = item.message.thread_id;

    if (item.thread_name) {
      const badge = document.createElement("div");
      badge.className = "scrapbook-message-thread";
      badge.textContent = `📁 ${item.thread_name}`;
      div.appendChild(badge);
    }

    const body = document.createElement("div");
    body.textContent = item.message.body ?? "(no text)";
    div.appendChild(body);

    const ts = messageSortTs(item.message);
    if (ts) {
      const meta = document.createElement("div");
      meta.className = "meta";
      meta.textContent = new Date(ts).toLocaleString();
      div.appendChild(meta);
    }

    div.addEventListener("click", () => {
      void jumpToMessageInThread(item.message.id, item.message.thread_id);
    });

    frag.appendChild(div);
    previousThreadId = item.message.thread_id;
    lastThreadIdInFragment = item.message.thread_id;
  });

  if (mode === "prepend" && existingFirstThreadId && lastThreadIdInFragment && existingFirstThreadId !== lastThreadIdInFragment) {
    appendSeparator();
  }

  mode === "prepend" ? scrapbookMessageList.prepend(frag) : scrapbookMessageList.appendChild(frag);
}

async function jumpToMessageInThread(messageId: string, threadId: string) {
  setContentPane("messages");
  currentThreadId = threadId;
  highlightMessageId = messageId;

  // Update thread selection in UI
  document.querySelectorAll("#thread-list li").forEach((el) => el.classList.remove("active"));
  const threadEl = document.querySelector(`#thread-list li[data-thread-id="${threadId}"]`);
  threadEl?.classList.add("active");

  // Check if message is already loaded
  if (messageStore.some((m) => m.id === messageId)) {
    scrollToMessageTop(messageId);
    return;
  }

  // Load messages around the target message
  try {
    const context = await apiListMessagesAround(messageId, 40, 40);

    if (context.length === 0) return;

    const oldest = context[0];
    const newest = context[context.length - 1];
    currentBeforeTs = oldest ? messageSortTs(oldest) : null;
    currentBeforeId = oldest?.id ?? null;
    currentAfterTs = newest ? messageSortTs(newest) : null;
    currentAfterId = newest?.id ?? null;
    messageStore = context;
    renderMessages(context, "replace");
    void fetchReactionsForMessages(context.map((m) => m.id));
    void fetchTagsForMessages(context.map((m) => m.id));
    scrollToMessageTop(messageId);
  } catch (err) {
    if (statusEl) statusEl.textContent = `Error loading message: ${err}`;
  }
}

// Tag manager event listeners
manageTagsBtn?.addEventListener("click", () => {
  tagManagerModal?.classList.remove("hidden");
  renderTagManager();
});

closeTagManager?.addEventListener("click", () => {
  tagManagerModal?.classList.add("hidden");
});

tagManagerModal?.addEventListener("click", (e) => {
  if (e.target === tagManagerModal) {
    tagManagerModal.classList.add("hidden");
  }
});

createTagBtn?.addEventListener("click", () => {
  const name = newTagName?.value.trim() || "";
  const color = selectedTagColor || TAG_COLOR_PRESETS[0];
  if (!name) {
    alert("Enter a tag name");
    return;
  }
  void createTag(name, color);
  if (newTagName) newTagName.value = "";
});

initTagColorPresets();

refreshThreads().catch((err) => {
  if (statusEl) statusEl.textContent = `Error: ${err}`;
});

if (isTauri) {
  void refreshTags();
}

// Hide splash screen once app is loaded
window.addEventListener("load", () => {
  const splash = document.getElementById("splash-screen");
  if (splash) {
    // Add fade-out class to trigger transition
    splash.classList.add("fade-out");
    // Remove from DOM after animation completes
    setTimeout(() => {
      splash.classList.add("hidden");
    }, 300);
  }
});
