export {}

type McpApiResult<T = unknown> = {
  success: boolean
  status: number
  result?: T | null
  error?: unknown
  raw?: unknown
}

declare global {
  interface Window {
    mcpApi?: {
      health(baseUrl?: string): Promise<{
        success: boolean
        status: number
        body: unknown
      }>
      listTools(baseUrl?: string): Promise<{
        success: boolean
        status: number
        tools: unknown[]
        toolSpecs: unknown[]
        raw: unknown
      }>
      listResources(baseUrl?: string): Promise<{
        success: boolean
        status: number
        resources: unknown[]
        raw: unknown
      }>
      listIntegrations(baseUrl?: string): Promise<{
        success: boolean
        status: number
        integrations: unknown[]
        raw: unknown
      }>
      getIntegrationPreset(
        target: string,
        baseUrl?: string
      ): Promise<{
        success: boolean
        status: number
        preset: unknown
      }>
      rpcCall(
        method: string,
        params?: Record<string, unknown>,
        id?: string,
        baseUrl?: string
      ): Promise<McpApiResult>
      initialize(baseUrl?: string): Promise<McpApiResult>
      listToolsRpc(baseUrl?: string): Promise<McpApiResult>
      listResourcesRpc(baseUrl?: string): Promise<McpApiResult>
      readResourceRpc(uri: string, baseUrl?: string): Promise<McpApiResult>
      callToolHttp(
        toolName: string,
        args?: Record<string, unknown>,
        baseUrl?: string
      ): Promise<{
        success: boolean
        status: number
        code: string | null
        message: string | null
        result: unknown
      }>
      callToolRpc(
        toolName: string,
        args?: Record<string, unknown>,
        baseUrl?: string
      ): Promise<McpApiResult>
    }
  }
}
