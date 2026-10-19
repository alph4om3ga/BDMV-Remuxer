import type { Project } from './types';

export function retryProjectWithQueue(
  projectId: string,
  options: {
    retry: (projectId: string) => Promise<Project>;
    startMuxing: () => Promise<void>;
    getMuxerStatus: () => { is_running: boolean; is_paused: boolean };
    onRetry: (project: Project) => void;
  },
): Promise<void>;
