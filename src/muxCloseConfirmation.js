export function getMuxCloseConfirmation(isRunning, persistenceRetryRequired) {
  if (persistenceRetryRequired) {
    return {
      title: 'Retry workspace save',
      message: isRunning
        ? 'A previous close attempt could not save the workspace. Retry saving before closing? The current file will finish first.'
        : 'A previous close attempt could not save the workspace. Retry saving before closing?',
    };
  }

  return {
    title: 'Muxing in progress',
    message: 'Muxing is still in progress. The current file will finish first, and remaining work will be saved as Paused. Close the app?',
  };
}

export async function handleMuxCloseRequested(confirmPrompt, confirmClose, reportError) {
  if (!(await confirmPrompt())) return;

  try {
    await confirmClose();
  } catch (error) {
    reportError(String(error));
  }
}
