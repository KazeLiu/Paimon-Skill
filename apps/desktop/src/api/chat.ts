import { invoke } from "@tauri-apps/api/core";

import type { Conversation, Message, SendMessageResult } from "@/types/chat";

export const listConversations = () => invoke<Conversation[]>("list_conversations");
export const listMessages = (conversationId: string) =>
  invoke<Message[]>("list_messages", { conversationId });
export const renameConversation = (id: string, title: string) =>
  invoke<void>("rename_conversation", { id, title });
export const deleteConversation = (id: string) =>
  invoke<void>("delete_conversation", { id });
export const sendMessage = (input: {
  conversationId: string | null;
  providerId: string;
  content: string;
}) => invoke<SendMessageResult>("send_message", { input });
export const cancelRun = (runId: string) => invoke<boolean>("cancel_run", { runId });
