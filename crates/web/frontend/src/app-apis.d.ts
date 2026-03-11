export {};

type AnyAsyncApi = Record<string, (...args: any[]) => any>;

type ThemeSourceMode = "light" | "dark" | "system";

interface AppApiResult<T = unknown> {
  success: boolean;
  data?: T;
  error?: string;
}

type AppConfigResult<T = unknown> = AppApiResult<T>;

interface OpenDialogOptions {
  title?: string;
  properties?: string[];
  filters?: Array<{ name: string; extensions: string[] }>;
}

interface OpenDialogResult {
  canceled: boolean;
  filePaths: string[];
}

interface CacheDirectoryInfo {
  id: string;
  name: string;
  description: string;
  path: string;
  icon: string;
  canClear: boolean;
  size: number;
  fileCount: number;
  exists: boolean;
}

interface CacheInfo {
  baseDir: string;
  directories: CacheDirectoryInfo[];
  totalSize: number;
}

interface DataDirectoryInfo {
  path: string;
  isCustom: boolean;
}

interface CacheMutationResult {
  success: boolean;
  error?: string;
  path?: string | null;
}

interface ImportLogLookupResult {
  success: boolean;
  error?: string;
  path?: string | null;
}

interface ClipboardMutationResult {
  success: boolean;
  error?: string;
}

type EmbeddingServiceConfigSource = "reuse_llm" | "custom";

type EmbeddingMutationResult = {
  success: boolean;
  error?: string;
};

interface EmbeddingServiceConfig {
  id: string;
  name: string;
  apiSource: EmbeddingServiceConfigSource;
  model: string;
  baseUrl?: string;
  apiKey?: string;
  createdAt: number;
  updatedAt: number;
}

interface EmbeddingServiceConfigDisplay {
  id: string;
  name: string;
  apiSource: EmbeddingServiceConfigSource;
  model: string;
  baseUrl?: string;
  apiKey?: string;
  createdAt: number;
  updatedAt: number;
}

interface AppApi {
  send(channel: string, ...args: unknown[]): unknown;
  app: {
    getVersion(): Promise<string>;
    getAnalyticsEnabled(): Promise<boolean>;
    setAnalyticsEnabled(enabled: boolean): Promise<unknown>;
    checkUpdate(): Promise<unknown>;
    fetchRemoteConfig<T = unknown>(url: string): Promise<AppConfigResult<T>>;
    relaunch(): Promise<unknown>;
  };
  setThemeSource(mode: ThemeSourceMode): Promise<unknown>;
  dialog: {
    showOpenDialog(options: OpenDialogOptions): Promise<OpenDialogResult>;
  };
  clipboard: {
    copyImage(dataUrl: string): Promise<ClipboardMutationResult>;
  };
}

interface ElectronIpcRenderer {
  send(channel: string, ...args: unknown[]): void;
  invoke<T = unknown>(channel: string, ...args: unknown[]): Promise<T>;
  on(channel: string, listener: (...args: unknown[]) => void): void;
  sendSync<T = unknown>(channel: string, ...args: unknown[]): T;
}

interface ElectronBridge {
  ipcRenderer: ElectronIpcRenderer;
  webUtils?: {
    getPathForFile?(file: File): string | undefined;
  };
}

interface CacheApi {
  getInfo(): Promise<CacheInfo>;
  clear(cacheId: string): Promise<CacheMutationResult>;
  openDir(cacheId: string): Promise<unknown>;
  saveToDownloads(
    filename: string,
    imageData: string,
  ): Promise<CacheMutationResult>;
  getLatestImportLog(): Promise<ImportLogLookupResult>;
  getDataDir(): Promise<DataDirectoryInfo>;
  selectDataDir(): Promise<CacheMutationResult>;
  setDataDir(
    newDir: string | null,
    migrate: boolean,
  ): Promise<CacheMutationResult>;
  showInFolder(path: string): Promise<unknown>;
}

declare global {
  interface Window {
    api: AppApi;
    electron?: ElectronBridge;
    setThemeSource(mode: ThemeSourceMode): Promise<unknown>;
    chatApi: AnyAsyncApi;
    aiApi: AnyAsyncApi;
    llmApi: AnyAsyncApi;
    sessionApi: AnyAsyncApi;
    cacheApi: CacheApi;
    mergeApi: AnyAsyncApi;
    nlpApi: AnyAsyncApi;
    agentApi: AnyAsyncApi;
    embeddingApi: {
      getAllConfigs(): Promise<EmbeddingServiceConfigDisplay[]>;
      getConfig(id: string): Promise<EmbeddingServiceConfig | null>;
      getActiveConfigId(): Promise<string | null>;
      isEnabled(): Promise<boolean>;
      addConfig(
        config: Omit<EmbeddingServiceConfig, "id" | "createdAt" | "updatedAt">,
      ): Promise<EmbeddingMutationResult>;
      updateConfig(
        id: string,
        updates: Partial<
          Omit<EmbeddingServiceConfig, "id" | "createdAt" | "updatedAt">
        >,
      ): Promise<EmbeddingMutationResult>;
      deleteConfig(id: string): Promise<EmbeddingMutationResult>;
      setActiveConfig(id: string): Promise<EmbeddingMutationResult>;
      validateConfig(
        config: EmbeddingServiceConfig,
      ): Promise<EmbeddingMutationResult>;
      getVectorStoreStats(): Promise<{
        enabled: boolean;
        count?: number;
        sizeBytes?: number;
      }>;
      clearVectorStore(): Promise<EmbeddingMutationResult>;
    };
  }
}
