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
  createdAt: string;
}

export interface SendMessageResult {
  runId: string;
  conversation: Conversation;
  userMessage: Message;
}

export interface ChatEvent {
  kind: "message_delta" | "assistant_finished" | "run_failed" | "run_cancelled";
  runId: string;
  conversationId: string;
  delta: string | null;
  finalContent?: string | null;
  messageId: string | null;
  error: string | null;
}

export interface SourceRecord {
  title: string;
  url: string;
  sourceLevel: string;
  platform: string;
  capturedAt: string;
}

export interface ToolActivity {
  runId: string;
  toolName: string;
  status: string;
}
