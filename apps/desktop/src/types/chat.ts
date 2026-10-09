export interface Conversation {
  id: string;
  title: string;
  providerId: string;
  createdAt: string;
  updatedAt: string;
}

export interface Message {
  id: string;
  conversationId: string;
  role: string;
  content: string;
  attachments?: ChatAttachment[];
  runId?: string | null;
  status?: "streaming" | "error" | "cancelled";
  error?: string;
  retryable?: boolean;
  retryMessageId?: string;
  retryContent?: string;
  retryAttachments?: ChatAttachment[];
  segments?: AssistantSegment[];
  createdAt: string;
}

export interface AssistantSegment {
  id: string;
  kind: "text" | "tool";
  content: string;
  toolName?: string | null;
  status?: string | null;
  details?: string | null;
  url?: string | null;
  platform?: string | null;
}

export interface ChatAttachment {
  name: string;
  mimeType: string;
  dataUrl: string;
  size: number;
  kind: "image" | "file";
}

export interface FailedRunState {
  conversationId: string;
  providerId: string;
  userMessageId: string;
  content: string;
  attachments: ChatAttachment[];
  partialContent: string;
  error: string;
  runId?: string;
}

export interface SendMessageResult {
  runId: string;
  conversation: Conversation;
  userMessage: Message;
}

export interface ChatEvent {
  kind: "message_delta" | "assistant_finished" | "run_retrying" | "run_failed" | "run_cancelled";
  runId: string;
  conversationId: string;
  delta: string | null;
  finalContent?: string | null;
  messageId: string | null;
  error: string | null;
  segments?: AssistantSegment[] | null;
}

export interface SourceRecord {
  title: string;
  url: string;
  sourceLevel: string;
  platform: string;
  capturedAt: string;
}

export interface ChatSourceEvent {
  conversationId: string;
  runId: string;
  source: SourceRecord;
}

export interface StoredSourceRecord extends SourceRecord {
  id: string;
  conversationId: string;
  runId: string;
  callId: string;
  createdAt: string;
}

export interface PageLink {
  title: string;
  url: string;
}

export interface ToolResult {
  toolName: string;
  content: string;
  source: SourceRecord | null;
  links: PageLink[];
}

export interface ToolRunRecord {
  id: string;
  conversationId: string;
  runId: string;
  callId: string;
  toolName: string;
  arguments: string;
  status: string;
  result: ToolResult | null;
  error: string | null;
  createdAt: string;
}

export interface ToolActivity {
  conversationId: string;
  runId: string;
  toolName: string;
  status: string;
  createdAt?: string;
  details?: string;
  callId?: string;
  url?: string;
  platform?: string;
}

export interface ConversationStep {
  id: string;
  runId: string;
  kind: "tool" | "source";
  title: string;
  status: string;
  timestamp: string;
  details?: string;
  url?: string;
  platform?: string;
}
