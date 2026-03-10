export interface ChatMessage {
  text: string;
  type: "sent" | "received";
  name: string;
  avatar?: string;
}

export interface ModelInfo {
  name: string;
  description: string;
  size_gb: number;
  quality_score: number;
  filename: string;
  is_downloaded: boolean;
  is_active: boolean;
}

export interface SystemInfo {
  os: string;
  memory_total: number;
  memory_available: number;
  cpu: string;
  is_compatible: boolean;
  warnings: string[];
}
