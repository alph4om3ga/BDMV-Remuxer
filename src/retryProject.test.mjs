import assert from 'node:assert/strict';
import test from 'node:test';
import { retryProjectWithQueue } from './retryProject.js';

for (const [description, muxerStatus] of [
  ['while another project is processing', { is_running: true, is_paused: false }],
  ['while the queue is paused', { is_running: true, is_paused: true }],
]) {
  test(`retry ${description} queues the project without starting another runner`, async () => {
    const successfulFile = {
      path: '/disc/BDMV/STREAM/00001.m2ts',
      fingerprint: 'source-fingerprint',
    };
    const retriedProject = {
      id: 'failed-project',
      status: 'pending',
      completed_files: [successfulFile],
    };
    const calls = [];
    let queue = [{ id: 'active-project', status: 'processing' }, {
      id: retriedProject.id,
      status: 'error',
    }];

    await retryProjectWithQueue(retriedProject.id, {
      retry: async (projectId) => {
        calls.push(['retry', projectId]);
        return retriedProject;
      },
      startMuxing: async () => calls.push(['start']),
      getMuxerStatus: () => muxerStatus,
      onRetry: (project) => {
        queue = queue.map((item) => item.id === project.id ? project : item);
      },
    });

    assert.deepEqual(calls, [['retry', retriedProject.id]]);
    assert.equal(queue.find((project) => project.id === retriedProject.id).status, 'pending');
    assert.deepEqual(
      queue.find((project) => project.id === retriedProject.id).completed_files,
      [successfulFile],
    );
  });
}

test('a rejected retry does not update the visible queue or start muxing', async () => {
  const calls = [];
  let onRetryCalled = false;

  await assert.rejects(
    retryProjectWithQueue('failed-project', {
      retry: async () => {
        calls.push(['retry']);
        throw new Error('Retry was not accepted because the updated queue could not be saved.');
      },
      startMuxing: async () => calls.push(['start']),
      getMuxerStatus: () => ({ is_running: false, is_paused: false }),
      onRetry: () => {
        onRetryCalled = true;
      },
    }),
    /Retry was not accepted because the updated queue could not be saved/,
  );

  assert.deepEqual(calls, [['retry']]);
  assert.equal(onRetryCalled, false);
});

test('rapid repeated retries for the same project only enqueue and start once', async () => {
  const retriedProject = { id: 'failed-project', status: 'pending' };
  const calls = [];
  const retryingUpdates = [];
  let resolveRetry;
  const retryPromise = new Promise(resolve => {
    resolveRetry = resolve;
  });
  const options = {
    retry: async (projectId) => {
      calls.push(['retry', projectId]);
      return retryPromise;
    },
    startMuxing: async () => calls.push(['start']),
    getMuxerStatus: () => ({ is_running: false, is_paused: false }),
    onRetry: (project) => calls.push(['onRetry', project.id]),
    onRetrying: (projectId, isRetrying) =>
      retryingUpdates.push([projectId, isRetrying]),
  };

  const firstRetry = retryProjectWithQueue(retriedProject.id, options);
  const repeatedRetry = retryProjectWithQueue(retriedProject.id, options);

  assert.equal(await repeatedRetry, false);
  assert.deepEqual(calls, [['retry', retriedProject.id]]);
  assert.deepEqual(retryingUpdates, [[retriedProject.id, true]]);

  resolveRetry(retriedProject);
  assert.equal(await firstRetry, true);
  assert.deepEqual(calls, [
    ['retry', retriedProject.id],
    ['onRetry', retriedProject.id],
    ['start'],
  ]);
  assert.deepEqual(retryingUpdates, [
    [retriedProject.id, true],
    [retriedProject.id, false],
  ]);
});
