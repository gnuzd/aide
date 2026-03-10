import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage, ModelInfo, SystemInfo } from "./types";

class Store {
  isDownloading = $state(false);
  isModelLoaded = $state(false);
  models = $state<ModelInfo[]>([]);
  systemInfo = $state<SystemInfo | null>(null);

  messages: ChatMessage[] = $state([]);
  isThinking = $state<boolean>(false);

  async checkModel() {
    this.isModelLoaded = await invoke<boolean>("is_model_loaded");
    if (!this.isModelLoaded) {
      try {
        this.models = await invoke<ModelInfo[]>("get_models");
        this.systemInfo = await invoke<SystemInfo>("get_system_info");
      } catch (e) {
        console.error("Failed to load models/system info:", e);
      }
    } else {
      await this.initChat();
    }
  }

  async initChat() {
    try {
      const welcome = await invoke<string>("init_chat");
      this.messages.push({
        text: welcome,
        type: "received",
        name: "Aide",
      });
    } catch (e) {
      console.error("Init chat failed:", e);
    }
  }

  async downloadModel(filename: string) {
    this.isDownloading = true;
    try {
      await invoke("download_model", { filename });
      this.isModelLoaded = true;
    } catch (err) {
      alert("Download failed " + err);
    } finally {
      this.isDownloading = true;
    }
  }

  async sendMessage(message: string) {
    if (!message.trim()) return;

    this.messages.push({
      text: message,
      type: "sent",
      name: "You",
    });

    this.isThinking = true;
    let assistantMessage: ChatMessage = {
      text: "",
      type: "received",
      name: "Aide",
    };
    this.messages.push(assistantMessage);

    try {
      await invoke("send_message", { message });
    } catch (e) {
      // assistantMessage.text = "Error: " + e;
    } finally {
      this.isThinking = false;
    }
  }

  streaming(token: string) {
    const lastMessage = this.messages.at(-1);
    if (lastMessage && lastMessage.name === "Aide") {
      lastMessage.text += token;
    }
  }
}

const store = new Store();
export default store;
