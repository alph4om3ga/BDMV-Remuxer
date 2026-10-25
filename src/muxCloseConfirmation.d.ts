export function getMuxCloseConfirmation(
  isRunning: boolean,
  persistenceRetryRequired: boolean,
): {
  title: string;
  message: string;
};

export function handleMuxCloseRequested(
  confirmPrompt: () => Promise<boolean>,
  confirmClose: () => Promise<unknown>,
  reportError: (message: string) => void,
): Promise<void>;
