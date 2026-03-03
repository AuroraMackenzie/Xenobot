/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  // English engineering note.
  const component: DefineComponent<Record<string, never>, Record<string, never>, unknown>
  export default component
}
