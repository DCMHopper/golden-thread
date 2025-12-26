import type { MessageRow, ThreadMediaRow } from "./types";

export function messageSortTs(message: MessageRow): number {
  return message.sent_at ?? message.received_at ?? 0;
}

export function mediaSortTs(item: ThreadMediaRow): number {
  return item.sent_at ?? item.received_at ?? 0;
}

export function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#39;");
}

export function highlightBody(text: string, query: string): string {
  if (!query) return escapeHtml(text);
  const escaped = escapeHtml(text);
  const safeQuery = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const regex = new RegExp(safeQuery, "ig");
  return escaped.replace(regex, (match) => `<span class="match-text">${match}</span>`);
}

export function debounce<T extends (...args: any[]) => void>(fn: T, wait: number) {
  let timer: number | undefined;
  return (...args: Parameters<T>) => {
    if (timer) {
      window.clearTimeout(timer);
    }
    timer = window.setTimeout(() => {
      fn(...args);
    }, wait);
  };
}

export function throttleRaf<T extends (...args: any[]) => void>(fn: T) {
  let scheduled = false;
  return (...args: Parameters<T>) => {
    if (scheduled) return;
    scheduled = true;
    const run = () => {
      scheduled = false;
      fn(...args);
    };
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(run);
    } else {
      globalThis.setTimeout(run, 16);
    }
  };
}

export function setMediaSource(
  element: HTMLImageElement | HTMLVideoElement | HTMLAudioElement,
  src: string,
  mime?: string,
) {
  if (element instanceof HTMLImageElement) {
    element.src = src;
    return;
  }
  const source = document.createElement("source");
  source.src = src;
  if (mime) {
    source.type = mime;
  }
  element.innerHTML = "";
  element.appendChild(source);
  element.load();
}

export function createMediaPlaceholder(context: "gallery" | "message", label: string) {
  const div = document.createElement("div");
  div.className = `media-placeholder ${context}`;
  div.textContent = label;
  return div;
}

export function parseDateFromInput(input: HTMLInputElement | null, endOfDay: boolean): number | null {
  if (!input || !input.value) return null;

  // Flatpickr uses YYYY-MM-DD format
  const dateStr = input.value;
  const parts = dateStr.split("-");
  if (parts.length !== 3) return null;

  const year = parseInt(parts[0], 10);
  const month = parseInt(parts[1], 10);
  const day = parseInt(parts[2], 10);

  const date = new Date(year, month - 1, day, endOfDay ? 23 : 0, endOfDay ? 59 : 0, endOfDay ? 59 : 0, 0);
  return date.getTime();
}

export function sizeBucket(value?: string | null): number | null {
  if (value === "small") {
    return 0;
  }
  if (value === "medium") {
    return 1;
  }
  if (value === "large") {
    return 2;
  }
  return null;
}
