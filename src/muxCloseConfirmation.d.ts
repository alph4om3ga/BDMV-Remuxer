export function handleMuxCloseRequested(
  confirmPrompt: () => Promise<boolean>,
  confirmClose: () => Promise<unknown>,
  reportError: (message: string) => void,
): Promise<void>;
