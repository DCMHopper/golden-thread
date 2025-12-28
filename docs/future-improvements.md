# Future Improvements for Golden Thread

This document contains planned improvements and feature ideas for Golden Thread. These are organized by priority and category.

## High Impact UI/UX Improvements

### 1. Keyboard Shortcuts
**Priority:** High
**Effort:** Medium

Implement power user keyboard shortcuts for common actions:
- `Cmd/Ctrl+F` - Focus search bar
- `Cmd/Ctrl+J` - Open jump to date
- `Cmd/Ctrl+K` - Focus thread search
- `Arrow Up/Down` - Navigate between threads
- `Enter` - Open selected thread
- `Esc` - Close modals/lightbox/menus
- `Cmd/Ctrl+D` - Toggle dark mode
- `N/P` - Next/Previous search result (when search is active)

**Implementation Notes:**
- Add event listener on document for keydown events
- Check for modifier keys (Cmd/Ctrl)
- Prevent conflicts with browser shortcuts
- Add visual indicators (tooltips showing keyboard shortcuts)
- Make shortcuts configurable in options menu (future enhancement)

### 2. Loading States & Skeleton Screens
**Priority:** High
**Effort:** Medium

Add polished loading states throughout the app:
- Thread list skeleton while loading threads
- Message list skeleton while loading messages
- Shimmer/pulse animation on skeleton elements
- Smooth transitions when real content replaces skeletons
- Loading spinner for async operations (import, reset, etc.)

**Implementation Notes:**
- Create reusable skeleton components in CSS
- Use CSS animations for shimmer effect
- Add loading state management in TypeScript
- Ensure skeletons match actual content dimensions

**Files to modify:**
- `src/styles.css` - Add skeleton styles
- `src/main.ts` - Add loading state logic

### 3. Image Lightbox Improvements
**Priority:** Medium
**Effort:** Low-Medium

Enhance the current lightbox with:
- Zoom in/out controls (buttons + mousewheel)
- Pan/drag zoomed images
- Keyboard navigation (Left/Right arrows for prev/next image in thread)
- `Esc` to close
- Download button for current image
- Image counter ("3 of 15")
- Smooth transitions between images

**Implementation Notes:**
- Add zoom state management
- Implement pan with mouse drag events
- Add keyboard event listeners
- Consider using CSS transforms for smooth zoom/pan
- Add download functionality using Tauri file system APIs

**Files to modify:**
- `src/styles.css` - Lightbox controls styling
- `src/main.ts` - Lightbox interaction logic

## Nice Quality of Life Features

### 4. Thread Pinning
**Priority:** Medium
**Effort:** Low

Allow users to pin important conversations to the top:
- Star/pin icon next to thread name
- Pinned threads appear at top of list with visual indicator
- Persist pinned state in database
- Quick toggle on/off

**Implementation Notes:**
- Add `pinned` boolean field to threads table
- Modify thread list query to sort pinned first
- Add star icon button in thread list items
- Update CSS for pinned thread styling

**Files to modify:**
- Database schema (add pinned column)
- `src/main.ts` - Thread rendering and pin toggle logic
- `src/styles.css` - Pinned thread styling

### 5. Search Within Thread
**Priority:** Medium
**Effort:** Low-Medium

Add ability to filter/search messages within the currently open thread:
- Small search input in message view header
- Highlight matching messages
- Scroll to first match
- Count of matches ("5 matches")
- Clear button

**Implementation Notes:**
- Add search input to message view
- Filter messages client-side for current thread
- Use existing search highlighting logic
- Maintain separate state from global search

**Files to modify:**
- `index.html` - Add search input to message view
- `src/main.ts` - Thread-specific search logic
- `src/styles.css` - Style thread search input

### 6. Export Thread
**Priority:** Low-Medium
**Effort:** Medium

## Performance & Media Handling

### Thumbnail Warmup (Deferred)
**Priority:** Medium
**Effort:** Medium

We experimented with post-import thumbnail warmup (precompute encrypted thumbnails to smooth gallery scrolling), but deferred it due to high CPU usage and long runtimes on large archives. Current approach favors on-demand thumbnail generation with concurrency caps and in-memory caching for smooth scrolling without sustained background load.

**Context & decision:**
- Warmup improves first-load gallery performance.
- On large archives it can peg CPU for a long time and gets interrupted by app quit.
- We chose to disable warmup for now and revisit after other performance work stabilizes.

**Future options:**
- Add progress/status UI and throttle CPU.
- Limit warmup to recent media or run only while idle.
- Make warmup user-configurable.

Export a conversation as a file:
- Export as HTML (styled, self-contained)
- Export as plain text
- Export as JSON (for technical users)
- Date range selection for export
- Include/exclude media option
- Save dialog with filename suggestion

**Implementation Notes:**
- Use Tauri dialog plugin for save file picker
- Generate HTML with embedded CSS
- Format plain text with timestamps and sender names
- Handle media export (embed or link)
- Add export button to thread view or options menu

**Files to modify:**
- `src/main.ts` - Export logic and formatting
- `index.html` - Add export button/menu item
- `src-tauri/` - Tauri commands for file writing

## Performance Optimizations

### 7. Virtual Scrolling
**Priority:** Low (only needed for very large threads)
**Effort:** High

Implement virtual scrolling for message lists with thousands of messages:
- Only render visible messages + small buffer
- Calculate scroll position based on message heights
- Maintain scroll position when loading more
- Smooth scrolling experience

**Implementation Notes:**
- Consider using a library (e.g., `virtual-scroller` or custom implementation)
- Estimate message heights or measure after render
- Update scroll calculations when window resizes
- Test with threads of 10,000+ messages

**Alternative:** Pagination (simpler but less smooth UX)

### 8. Lazy Load Images
**Priority:** Medium
**Effort:** Low-Medium

Load images as they scroll into view:
- Use Intersection Observer API
- Placeholder image/icon while loading
- Progressive loading (low-res then high-res)
- Preload images slightly before visible

**Implementation Notes:**
- Add Intersection Observer to message rendering
- Use `data-src` attribute for lazy images
- Replace with actual `src` when in viewport
- Handle loading errors gracefully

**Files to modify:**
- `src/main.ts` - Message rendering and lazy load logic

## Accessibility Improvements

### 9. Enhanced Accessibility
**Priority:** Medium
**Effort:** Medium

Improve screen reader support and keyboard navigation:
- Proper ARIA labels on all interactive elements
- ARIA live regions for dynamic content updates
- Focus management (trap in modals, restore after close)
- Skip links for keyboard users
- High contrast mode (beyond dark mode)
- Reduced motion mode (respect `prefers-reduced-motion`)

**Implementation Notes:**
- Audit with screen reader (VoiceOver, NVDA)
- Add `aria-label`, `aria-describedby` where needed
- Implement focus trap for modals
- Test all features with keyboard only
- Add CSS for `prefers-reduced-motion` and `prefers-contrast`

**Files to modify:**
- `index.html` - Add ARIA attributes
- `src/styles.css` - Add accessibility media queries
- `src/main.ts` - Focus management logic

## UI Polish

### 10. Toast Notifications
**Priority:** Low-Medium
**Effort:** Low

Add non-intrusive notifications for user actions:
- "Thread exported successfully"
- "Preferences saved"
- "Import complete"
- Auto-dismiss after 3-5 seconds
- Dismiss on click
- Queue multiple toasts
- Different styles for success/error/info

**Implementation Notes:**
- Create toast container and styling
- Add toast queue management
- Animate in/out with CSS transitions
- Position in bottom-right or top-right
- Ensure accessible (ARIA live region)

**Files to modify:**
- `index.html` - Add toast container
- `src/styles.css` - Toast styling and animations
- `src/main.ts` - Toast queue and display logic

### 11. Smooth Scroll to Date/Message
**Priority:** Low
**Effort:** Low

Add smooth scrolling animations:
- When jumping to a date, smooth scroll to that message
- When clicking search results, smooth scroll to match
- Highlight target message briefly
- Respect `prefers-reduced-motion`

**Implementation Notes:**
- Use `scrollIntoView({ behavior: 'smooth' })`
- Add temporary highlight class to target message
- Check `prefers-reduced-motion` media query

**Files to modify:**
- `src/main.ts` - Update jump and search scroll logic
- `src/styles.css` - Add highlight animation

## Statistics & Analytics

### 12. Message Statistics Dashboard
**Priority:** Low
**Effort:** High

Add a statistics view showing:
- Total messages over time (line chart)
- Messages per contact/thread (bar chart)
- Most active days/hours (heatmap)
- Media breakdown (photos vs videos vs files)
- Conversation length statistics
- Word cloud of frequent terms

**Implementation Notes:**
- Add new tab/view for statistics
- Use a charting library (Chart.js, D3.js, or lightweight alternative)
- Query database for aggregated statistics
- Make it performant for large archives
- Add date range filters

**Files to modify:**
- `index.html` - Add statistics tab/view
- `src/main.ts` - Statistics query and rendering logic
- `src/styles.css` - Statistics view styling
- `src-tauri/` - Database queries for stats

---

## Notes for Future Implementation

**General Guidelines:**
- Maintain the warm, cozy aesthetic established in the UI redesign
- Ensure all new features work in both light and dark modes
- Test with all four theme colors (amber, blue, purple, green)
- Follow the existing 8px spacing grid and design system
- Prioritize accessibility and keyboard navigation
- Keep the app lightweight and fast

**Testing Checklist for New Features:**
- [ ] Works in light mode
- [ ] Works in dark mode
- [ ] Works with all 4 accent colors
- [ ] Keyboard accessible
- [ ] Screen reader friendly
- [ ] Responsive to window resizing
- [ ] Persists preferences correctly
- [ ] No console errors
- [ ] Build succeeds
- [ ] Performance tested with large datasets

**Current Tech Stack:**
- Frontend: TypeScript + Vite
- UI: Vanilla HTML/CSS (no framework)
- Desktop: Tauri 2.0
- Date Picker: Flatpickr
- Fonts: Inter + Noto Sans (CJK/Cyrillic support)
