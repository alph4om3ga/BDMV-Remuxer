const retryingProjectIds = new Set();

export async function retryProjectWithQueue(projectId, {
  retry,
  startMuxing,
  getMuxerStatus,
  onRetry,
  onRetrying,
}) {
  if (retryingProjectIds.has(projectId)) return false;

  retryingProjectIds.add(projectId);
  try {
    onRetrying?.(projectId, true);
    const project = await retry(projectId);
    onRetry(project);

    if (!getMuxerStatus().is_running) {
      await startMuxing();
    }
    return true;
  } finally {
    retryingProjectIds.delete(projectId);
    onRetrying?.(projectId, false);
  }
}
