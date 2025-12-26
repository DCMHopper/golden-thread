export type ThreadSummary = {
  id: string;
  name?: string | null;
  last_message_at?: number | null;
  message_count: number;
};

export type MessageRow = {
  id: string;
  thread_id: string;
  sender_id?: string | null;
  sent_at?: number | null;
  received_at?: number | null;
  message_type: string;
  body?: string | null;
  is_outgoing: boolean;
  is_view_once: boolean;
  quote_message_id?: string | null;
  metadata_json?: string | null;
};

export type SearchHit = {
  message: MessageRow;
  rank: number;
};

export type ReactionSummary = {
  message_id: string;
  emoji: string;
  count: number;
};

export type AttachmentRow = {
  id: string;
  message_id: string;
  sha256: string;
  mime?: string | null;
  size_bytes?: number | null;
  original_filename?: string | null;
  kind?: string | null;
  width?: number | null;
  height?: number | null;
  duration_ms?: number | null;
};

export type ThreadMediaRow = {
  id: string;
  message_id: string;
  thread_id: string;
  sha256: string;
  mime?: string | null;
  size_bytes?: number | null;
  original_filename?: string | null;
  kind?: string | null;
  width?: number | null;
  height?: number | null;
  duration_ms?: number | null;
  sent_at?: number | null;
  received_at?: number | null;
};

export type MediaAsset = {
  id: string;
  sha256: string;
  mime?: string | null;
  size_bytes?: number | null;
  original_filename?: string | null;
  kind?: string | null;
  width?: number | null;
  height?: number | null;
  duration_ms?: number | null;
};

export type SourceInfo = {
  src: string;
  via: "file" | "data";
};

export type Tag = {
  id: string;
  name: string;
  color: string;
  created_at: number;
  display_order: number;
};

export type MessageTags = {
  message_id: string;
  tags: Tag[];
};

export type ScrapbookMessage = {
  message: MessageRow;
  thread_name?: string | null;
  is_discontinuous: boolean;
};
