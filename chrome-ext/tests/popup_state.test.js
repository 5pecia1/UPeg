'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  beginReconnect,
  beginDispatch,
  enableDispatchRun,
  isCurrentConnection,
  isCurrentDispatch,
} = require('../popup_state.js');

test('a_delayed_board_load_cannot_apply_after_reconnect', async () => {
  const state = {
    connectionEpoch: 0,
    toolsByBoard: new Map([['old', ['cached']]]),
    activeBoardIndex: 2,
    activeToolIndex: 3,
  };
  const oldEpoch = beginReconnect(state);
  const delayed = Promise.resolve(['old result']);
  const newEpoch = beginReconnect(state);
  await delayed;

  assert.equal(isCurrentConnection(state, oldEpoch), false);
  assert.equal(isCurrentConnection(state, newEpoch), true);
  assert.equal(state.toolsByBoard.size, 0);
  assert.equal(state.activeBoardIndex, 0);
  assert.equal(state.activeToolIndex, 0);
});

test('a_delayed_connection_failure_cannot_replace_new_connection_ui', async () => {
  const state = { connectionEpoch: 0, toolsByBoard: new Map(), activeBoardIndex: 0, activeToolIndex: 0 };
  const failedEpoch = beginReconnect(state);
  const delayedFailure = Promise.resolve(new Error('old endpoint failed'));
  const currentEpoch = beginReconnect(state);
  await delayedFailure;

  assert.equal(isCurrentConnection(state, failedEpoch), false);
  assert.equal(isCurrentConnection(state, currentEpoch), true);
});

test('a_delayed_dispatch_cannot_render_after_cancel_or_replacement', async () => {
  const state = { dispatch: null };
  const runButton = { disabled: true };
  const first = beginDispatch(state, { id: 'first' });
  const delayed = Promise.resolve();
  state.dispatch = null;
  // Opening replacement B owns its own enabled Run control before A settles.
  enableDispatchRun(runButton);
  assert.equal(runButton.disabled, false);
  await delayed;
  assert.equal(isCurrentDispatch(state, first), false);
  assert.equal(runButton.disabled, false, 'stale A must not disable replacement B');

  const second = beginDispatch(state, { id: 'second' });
  assert.equal(isCurrentDispatch(state, first), false);
  assert.equal(isCurrentDispatch(state, second), true);
});

test('a_delayed_file_read_cannot_submit_after_its_form_session_changes', async () => {
  const state = { dispatch: null };
  const readingSession = beginDispatch(state, { id: 'A' });
  const fileRead = Promise.resolve({ upload: 'bytes' });
  const replacement = beginDispatch(state, { id: 'B' });
  await fileRead;

  assert.equal(isCurrentDispatch(state, readingSession), false);
  assert.equal(isCurrentDispatch(state, replacement), true);
});
