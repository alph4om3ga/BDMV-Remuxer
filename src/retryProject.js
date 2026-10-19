export async function retryProjectWithQueue(projectId, {
  retry,
  startMuxing,
  getMuxerStatus,
  onRetry,
}) {
  const project = await retry(projectId);
  onRetry(project);

  if (!getMuxerStatus().is_running) {
    await startMuxing();
  }
}
