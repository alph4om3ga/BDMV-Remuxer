import assert from 'node:assert/strict';
import test from 'node:test';
import {
  getMuxCloseConfirmation,
  handleMuxCloseRequested,
} from './muxCloseConfirmation.js';

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

test('a failed save is reported and a later close attempt can succeed', async () => {
  let closeCommandCalls = 0;
  const reportedErrors = [];

  const requestClose = () => handleMuxCloseRequested(
    async () => true,
    async () => {
      closeCommandCalls += 1;
      if (closeCommandCalls <= 2) throw new Error('workspace save failed');
    },
    (message) => reportedErrors.push(message),
  );

  await requestClose();
  assert.equal(closeCommandCalls, 1);
  assert.deepEqual(reportedErrors, ['Error: workspace save failed']);

  await requestClose();
  assert.equal(closeCommandCalls, 2);
  assert.deepEqual(reportedErrors, [
    'Error: workspace save failed',
    'Error: workspace save failed',
  ]);

  await requestClose();
  assert.equal(closeCommandCalls, 3);
  assert.deepEqual(reportedErrors, [
    'Error: workspace save failed',
    'Error: workspace save failed',
  ]);
});

test('a failed close save prompts to retry workspace persistence after muxing stops', () => {
  assert.deepEqual(getMuxCloseConfirmation(false, true), {
    title: 'Retry workspace save',
    message: 'A previous close attempt could not save the workspace. Retry saving before closing?',
  });
});

test('a failed close save while muxing also explains that the current file will finish', () => {
  assert.deepEqual(getMuxCloseConfirmation(true, true), {
    title: 'Retry workspace save',
    message: 'A previous close attempt could not save the workspace. Retry saving before closing? The current file will finish first.',
  });
});

test('an active muxer close still explains that the current file will finish', () => {
  assert.deepEqual(getMuxCloseConfirmation(true, false), {
    title: 'Muxing in progress',
    message: 'Muxing is still in progress. The current file will finish first, and remaining work will be saved as Paused. Close the app?',
  });
});
