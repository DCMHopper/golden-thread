/**
 * Shared application state module.
 * Centralizes all module-level state from main.ts.
 */

import type {
  AttachmentRow,
  MediaAsset,
  MessageRow,
  ReactionSummary,
  ScrapbookMessage,
  SearchHit,
  Tag,
  ThreadMediaRow,
  ThreadSummary,
} from "./types";
import {
  ATTACHMENT_DATA_URL_CACHE_MAX,
  ATTACHMENT_FILE_URL_CACHE_MAX,
  ATTACHMENT_LIST_CACHE_MAX,
  ATTACHMENT_THUMB_CACHE_MAX_BYTES,
} from "./constants";
import { LruCache, WeightedLruCache } from "./cache";

// --- Import Panel State ---
export let selectedBackupPath: string | null = null;
export let isBusy = false;

export function setSelectedBackupPath(path: string | null) {
  selectedBackupPath = path;
}

export function setIsBusy(busy: boolean) {
  isBusy = busy;
}

// --- Thread State ---
export let threadStore: ThreadSummary[] = [];
export let currentThreadId: string | null = null;
export let activeThreadId: string | null = null;
export let activeThreadEl: HTMLLIElement | null = null;
export const threadScrollPositions = new Map<string, number>();

export function setThreadStore(threads: ThreadSummary[]) {
  threadStore = threads;
}

export function setCurrentThreadId(id: string | null) {
  currentThreadId = id;
}

export function setActiveThreadId(id: string | null) {
  activeThreadId = id;
}

export function setActiveThreadEl(el: HTMLLIElement | null) {
  activeThreadEl = el;
}

// --- Message State ---
export let messageStore: MessageRow[] = [];
export const messageById = new Map<string, MessageRow>();
export let currentBeforeTs: number | null = null;
export let currentBeforeId: string | null = null;
export let currentAfterTs: number | null = null;
export let currentAfterId: string | null = null;
export let isLoadingMessages = false;
export let highlightMessageId: string | null = null;
export let viewportAnchor: { id: string; offset: number } | null = null;
export let anchorScheduled = false;
export let messagesRequestId = 0;
export let messagesAbortController: AbortController | null = null;

export function setMessageStore(messages: MessageRow[]) {
  messageStore = messages;
}

export function setCurrentBeforeTs(ts: number | null) {
  currentBeforeTs = ts;
}

export function setCurrentBeforeId(id: string | null) {
  currentBeforeId = id;
}

export function setCurrentAfterTs(ts: number | null) {
  currentAfterTs = ts;
}

export function setCurrentAfterId(id: string | null) {
  currentAfterId = id;
}

export function setIsLoadingMessages(loading: boolean) {
  isLoadingMessages = loading;
}

export function setHighlightMessageId(id: string | null) {
  highlightMessageId = id;
}

export function setViewportAnchor(anchor: { id: string; offset: number } | null) {
  viewportAnchor = anchor;
}

export function setAnchorScheduled(scheduled: boolean) {
  anchorScheduled = scheduled;
}

export function incrementMessagesRequestId(): number {
  return ++messagesRequestId;
}

export function setMessagesAbortController(controller: AbortController | null) {
  messagesAbortController = controller;
}

/**
 * Creates a new AbortController for message loading, canceling any previous one.
 * Returns the new controller.
 */
export function createMessagesAbortController(): AbortController {
  // Abort any previous in-flight request
  if (messagesAbortController) {
    messagesAbortController.abort();
  }
  const controller = new AbortController();
  messagesAbortController = controller;
  return controller;
}

// --- Search State ---
export let searchHits: SearchHit[] = [];
export let searchIndex = -1;
export let searchQuery = "";
export let searchMatchIds = new Set<string>();
export let isSearchJumping = false;
export let searchRequestId = 0;

export function setSearchHits(hits: SearchHit[]) {
  searchHits = hits;
}

export function setSearchIndex(index: number) {
  searchIndex = index;
}

export function setSearchQuery(query: string) {
  searchQuery = query;
}

export function setSearchMatchIds(ids: Set<string>) {
  searchMatchIds = ids;
}

export function setIsSearchJumping(jumping: boolean) {
  isSearchJumping = jumping;
}

export function incrementSearchRequestId(): number {
  return ++searchRequestId;
}

// --- Gallery State ---
export let galleryItems: ThreadMediaRow[] = [];
export const galleryItemsById = new Map<string, ThreadMediaRow>();
export let galleryThumbObserver: IntersectionObserver | null = null;
export const galleryThumbQueue = new Map<HTMLImageElement, MediaAsset>();
export let galleryThumbInFlight = 0;
export const galleryThumbTasks: Array<() => void> = [];
export const galleryThumbPending = new Set<string>();
export let galleryEvictionTimer: number | null = null;
export let galleryOffset = 0;
export let galleryLoading = false;
export let galleryHasMore = true;
export let galleryRequestId = 0;

export function setGalleryItems(items: ThreadMediaRow[]) {
  galleryItems = items;
}

export function setGalleryThumbObserver(observer: IntersectionObserver | null) {
  galleryThumbObserver = observer;
}

export function setGalleryThumbInFlight(count: number) {
  galleryThumbInFlight = count;
}

export function setGalleryEvictionTimer(timer: number | null) {
  galleryEvictionTimer = timer;
}

export function setGalleryOffset(offset: number) {
  galleryOffset = offset;
}

export function setGalleryLoading(loading: boolean) {
  galleryLoading = loading;
}

export function setGalleryHasMore(hasMore: boolean) {
  galleryHasMore = hasMore;
}

export function incrementGalleryRequestId(): number {
  return ++galleryRequestId;
}

// --- Lightbox State ---
export let lightboxGallery: MediaAsset[] = [];
export let lightboxIndex = -1;
export let isLightboxFullscreen = false;
export let lightboxEl: HTMLDivElement | null = null;
export let lightboxContent: HTMLDivElement | null = null;
export let lightboxClose: HTMLButtonElement | null = null;

export function setLightboxGallery(gallery: MediaAsset[]) {
  lightboxGallery = gallery;
}

export function setLightboxIndex(index: number) {
  lightboxIndex = index;
}

export function setIsLightboxFullscreen(fullscreen: boolean) {
  isLightboxFullscreen = fullscreen;
}

export function setLightboxEl(el: HTMLDivElement | null) {
  lightboxEl = el;
}

export function setLightboxContent(el: HTMLDivElement | null) {
  lightboxContent = el;
}

export function setLightboxClose(el: HTMLButtonElement | null) {
  lightboxClose = el;
}

// --- Scrapbook State ---
export let tagsStore: Tag[] = [];
export let messageTagsCache = new Map<string, Tag[]>();
export let currentScrapbookTagId: string | null = null;
export let scrapbookBeforeTs: number | null = null;
export let scrapbookBeforeId: string | null = null;
export let scrapbookMessages: ScrapbookMessage[] = [];
export let isLoadingScrapbook = false;
export const scrapbookScrollPositions = new Map<string, number>();
export let scrapbookRequestId = 0;

export function setTagsStore(tags: Tag[]) {
  tagsStore = tags;
}

export function clearMessageTagsCache() {
  messageTagsCache = new Map();
}

export function setCurrentScrapbookTagId(id: string | null) {
  currentScrapbookTagId = id;
}

export function setScrapbookBeforeTs(ts: number | null) {
  scrapbookBeforeTs = ts;
}

export function setScrapbookBeforeId(id: string | null) {
  scrapbookBeforeId = id;
}

export function setScrapbookMessages(messages: ScrapbookMessage[]) {
  scrapbookMessages = messages;
}

export function setIsLoadingScrapbook(loading: boolean) {
  isLoadingScrapbook = loading;
}

export function incrementScrapbookRequestId(): number {
  return ++scrapbookRequestId;
}

// --- UI State ---
export let currentPane: "messages" | "search" | "gallery" | "scrapbook" = "messages";
export let requireMediaClick = true;

export function setCurrentPane(pane: "messages" | "search" | "gallery" | "scrapbook") {
  currentPane = pane;
}

export function setRequireMediaClick(require: boolean) {
  requireMediaClick = require;
}

// --- Reset Confirm State ---
export let resetConfirmState: "initial" | "confirming" = "initial";
export let resetConfirmTimeout: number | null = null;

export function setResetConfirmState(state: "initial" | "confirming") {
  resetConfirmState = state;
}

export function setResetConfirmTimeout(timeout: number | null) {
  resetConfirmTimeout = timeout;
}

// --- Tag Delete Confirm State ---
export let deleteTagConfirmId: string | null = null;
export let deleteTagConfirmTimeout: number | null = null;
export let selectedTagColor = "#c3684a";

export function setDeleteTagConfirmId(id: string | null) {
  deleteTagConfirmId = id;
}

export function setDeleteTagConfirmTimeout(timeout: number | null) {
  deleteTagConfirmTimeout = timeout;
}

export function setSelectedTagColor(color: string) {
  selectedTagColor = color;
}

// --- Caches ---
export const attachmentCache = new LruCache<string, AttachmentRow[]>(ATTACHMENT_LIST_CACHE_MAX);
export const attachmentDataCache = new LruCache<string, string>(ATTACHMENT_DATA_URL_CACHE_MAX);
export const attachmentFileCache = new LruCache<string, string>(ATTACHMENT_FILE_URL_CACHE_MAX);
export const attachmentThumbCache = new WeightedLruCache<string, string>(
  ATTACHMENT_THUMB_CACHE_MAX_BYTES,
  (value) => value.length,
);
export const reactionMap = new Map<string, ReactionSummary[]>();
export const messageAttachmentsCache = new Map<string, MediaAsset[]>();
export const threadMediaCache = new Map<string, MediaAsset[]>();
export const attachmentsById = new Map<string, MediaAsset>();

// --- Task Queues State ---
export let thumbInFlight = 0;
export const thumbQueue: Array<() => void> = [];

export function setThumbInFlight(count: number) {
  thumbInFlight = count;
}

// --- Constants ---
export const THUMB_CONCURRENCY = 4;
export const LARGE_MEDIA_BYTES = 10 * 1024 * 1024;

// --- Tauri Detection ---
export const isTauri = typeof (window as any).__TAURI_INTERNALS__ !== "undefined";
