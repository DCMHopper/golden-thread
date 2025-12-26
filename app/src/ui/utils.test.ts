import { describe, expect, it, vi } from "vitest";
import { escapeHtml, highlightBody, messageSortTs, throttleRaf } from "./utils";

describe("utils", () => {
  it("messageSortTs prefers sent_at then received_at", () => {
    expect(messageSortTs({ sent_at: 5, received_at: 10 } as any)).toBe(5);
    expect(messageSortTs({ sent_at: null, received_at: 10 } as any)).toBe(10);
    expect(messageSortTs({ sent_at: undefined, received_at: undefined } as any)).toBe(0);
  });

  it("escapeHtml escapes reserved characters", () => {
    expect(escapeHtml(`&<>"'`)).toBe("&amp;&lt;&gt;&quot;&#39;");
  });

  it("highlightBody wraps matches and escapes text", () => {
    const result = highlightBody("Hello <b>world</b>", "world");
    expect(result).toContain("&lt;b&gt;");
    expect(result).toContain('<span class="match-text">world</span>');
  });

  it("throttleRaf coalesces calls", () => {
    vi.useFakeTimers();
    const calls: number[] = [];
    const throttled = throttleRaf((value: number) => {
      calls.push(value);
    });
    throttled(1);
    throttled(2);
    vi.advanceTimersByTime(20);
    throttled(3);
    vi.advanceTimersByTime(20);
    expect(calls).toEqual([1, 3]);
    vi.useRealTimers();
  });
});
