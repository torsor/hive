/// <reference types="vite/client" />

type KatexApi = {
  renderToString(tex: string, options?: Record<string, unknown>): string;
};

declare module "./chat-render.js" {
  export function escapeHtml(text: string): string;
  export function renderMarkdownWithMath(
    text: string | null | undefined,
    katex: KatexApi | null | undefined,
  ): string;
}

interface Window {
  katex?: KatexApi;
  requestIdleCallback?: (
    callback: () => void,
    opts?: { timeout: number },
  ) => number;
  cancelIdleCallback?: (id: number) => void;
}
