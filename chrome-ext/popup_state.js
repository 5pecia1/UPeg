'use strict';

// Small race guards for popup-owned asynchronous work. A reconnect or a
// canceled/replaced form invalidates earlier async completions.
const UpegPopupState = (() => {
  function beginReconnect(state) {
    state.connectionEpoch += 1;
    state.toolsByBoard.clear();
    state.activeBoardIndex = 0;
    state.activeToolIndex = 0;
    return state.connectionEpoch;
  }

  function isCurrentConnection(state, epoch) {
    return state.connectionEpoch === epoch;
  }

  function beginDispatch(state, session) {
    state.dispatch = session;
    return session;
  }

  function isCurrentDispatch(state, session) {
    return state.dispatch === session;
  }

  function enableDispatchRun(button) {
    button.disabled = false;
  }

  return Object.freeze({
    beginReconnect,
    beginDispatch,
    enableDispatchRun,
    isCurrentConnection,
    isCurrentDispatch,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegPopupState;
}
