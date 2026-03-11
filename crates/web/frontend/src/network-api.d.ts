export {};

type ProxyMode = "off" | "system" | "manual";

declare global {
  interface Window {
    networkApi: {
      getProxyConfig(): Promise<{
        mode: ProxyMode | string;
        url?: string;
      }>;
      saveProxyConfig(config: {
        mode: ProxyMode | string;
        url?: string;
      }): Promise<{
        success: boolean;
        error?: string;
      }>;
      testProxyConnection(proxyUrl: string): Promise<{
        success: boolean;
        error?: string;
      }>;
      getRuntimeHealth(): Promise<{
        success: boolean;
        status: number;
        body: unknown;
      }>;
      getRuntimeStatus(): Promise<{
        success: boolean;
        status: number;
        body: unknown;
      }>;
      getServiceIndex(): Promise<{
        success: boolean;
        status: number;
        body: unknown;
      }>;
      getSandboxDoctor(fileGatewayDir?: string): Promise<{
        success: boolean;
        status: number;
        report: unknown;
        raw: unknown;
      }>;
    };
  }
}
