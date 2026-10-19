export async function handleMuxCloseRequested(confirmPrompt, confirmClose, reportError) {
  if (!(await confirmPrompt())) return;

  try {
    await confirmClose();
  } catch (error) {
    reportError(String(error));
  }
}
