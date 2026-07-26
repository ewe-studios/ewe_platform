// E2E test harness — loaded via ScriptInjector into every WebView.
// Each test fires on a staggered setTimeout so results land in logcat in order.
// Verify with: adb logcat | grep 'Tauri/Console.*E2E'
(function() {
  'use strict';
  if (typeof invokeIpc !== 'function') {
    console.error('[E2E] invokeIpc not available — skipping tests');
    return;
  }
  var tests = [
    { delay: 2000, name: 'present_modal', ipc: 'chrome', action: 'present_modal',
      payload: {url:'http://ewe.localhost/app/',style:'bottom_sheet',title:'Modal Test'} },
    { delay: 4000, name: 'show_dialog_webview', ipc: 'dialog', action: 'show',
      payload: {title:'Dialog Test', message:'Tauri WebView dialog', route:'http://ewe.localhost/app/'} },
    { delay: 6000, name: 'show_dialog_native', ipc: 'dialog', action: 'show',
      payload: {title:'Native Dialog',message:'Text-only AlertDialog',positive_button:'OK',negative_button:'Cancel'} },
  ];
  tests.forEach(function(t) {
    setTimeout(function() {
      console.error('[E2E] ' + t.name + '...');
      invokeIpc(t.ipc, t.action, t.payload)
        .then(function(r) { console.error('[E2E] ' + t.name + ' OK ' + JSON.stringify(r)); })
        .catch(function(e) { console.error('[E2E] ' + t.name + ' FAIL ' + (e.message || e)); });
    }, t.delay);
  });
})();
