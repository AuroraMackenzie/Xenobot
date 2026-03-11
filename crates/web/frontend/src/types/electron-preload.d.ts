declare module "@electron/preload/index" {
  export type EmbeddingServiceConfigSource = "reuse_llm" | "custom";

  export interface EmbeddingServiceConfig {
    id: string;
    name: string;
    apiSource: EmbeddingServiceConfigSource;
    model: string;
    baseUrl?: string;
    apiKey?: string;
    createdAt: number;
    updatedAt: number;
  }

  export interface EmbeddingServiceConfigDisplay {
    id: string;
    name: string;
    apiSource: EmbeddingServiceConfigSource;
    model: string;
    baseUrl?: string;
    apiKey?: string;
    createdAt: number;
    updatedAt: number;
  }
}
