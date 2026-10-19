import assert from 'node:assert/strict';
import test from 'node:test';
import { handleMuxCloseRequested } from './muxCloseConfirmation.js';

test('cancelling the close prompt does not stop muxing or approve closing', async () => {
  let closeCommandCalls = 0;
  let reportedErrors = 0;

  await handleMuxCloseRequested(
    async () => false,
    async () => { closeCommandCalls += 1; },
    () => { reportedErrors += 1; },
  );

  assert.equal(closeCommandCalls, 0);
  assert.equal(reportedErrors, 0);
});

test('accepting the close prompt invokes the save-and-close command', async () => {
  let closeCommandCalls = 0;

  await handleMuxCloseRequested(
    async () => true,
    async () => { closeCommandCalls += 1; },
    () => assert.fail('successful close must not report an error'),
  );

  assert.equal(closeCommandCalls, 1);
});

test('a failed save-and-close command is reported without treating the close as approved', async () => {
  let reportedError;

  await handleMuxCloseRequested(
    async () => true,
    async () => { throw new Error('workspace save failed'); },
    (message) => { reportedError = message; },
  );

  assert.equal(reportedError, 'Error: workspace save failed');
});
