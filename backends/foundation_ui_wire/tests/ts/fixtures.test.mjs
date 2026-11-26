import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
  BehaviourId,
  ByteBuffer,
  CapsStatus,
  ColumnData,
  ErrorCode,
  EventId,
  Frame,
  FrameKind,
  FrameMessage,
  MobileOperation,
  MountSurface,
  NativeCommandId,
  NodeKind,
  Platform,
  PropId,
  PropValue,
  SignedState,
  UiOperation,
  WebOperation,
} from '../../../foundation_wasm_ui/runtimes/foundation-ui-wire.js';

function fixture(name) {
  // Allocate an independent buffer so the file starts at byte zero.
  return Uint8Array.from(readFileSync(new URL(`../../fixtures/${name}.bin`, import.meta.url)));
}

function frame(name) {
  const buffer = new ByteBuffer(fixture(name));
  assert.ok(Frame.bufferHasIdentifier(buffer));
  const value = Frame.getRootAsFrame(buffer);
  assert.equal(value.schemaVersion(), 1);
  return value;
}

function frameMessage(name) {
  const value = frame(name);
  assert.equal(value.kindType(), FrameKind.FrameMessage);
  assert.equal(value.streamId(), 7n);
  assert.equal(value.sequenceId(), 20n);
  const message = value.kind(new FrameMessage());
  assert.equal(message.ack(), 5n);
  return message;
}

function onlyMessage(name) {
  const messages = frameMessage(name);
  assert.equal(messages.messagesLength(), 1);
  return messages.messages(0);
}

test('counter message sets the text on node 42', () => {
  const { operation } = onlyMessage('frame_ops_counter').unpack();
  assert.equal(operation.uiType, UiOperation.SetText);
  assert.equal(operation.ui.nodeId, 42);
  assert.equal(operation.ui.text, '1');
});

test('every operation form keeps its tag, group, and fields', () => {
  const groups = [
    ['ui', UiOperation, [
      ['CreateNode', { nodeId: 1, kind: NodeKind.TEXT }],
      ['SetText', { nodeId: 1, text: 'a' }],
      ['SetProp', { nodeId: 1, prop: PropId.ENABLED, valueType: PropValue.Bool, value: { v: true } }],
      ['AppendChild', { parentId: 1, childId: 2 }],
      ['InsertBefore', { parentId: 1, childId: 2, beforeId: 3 }],
      ['Remove', { nodeId: 2 }],
      ['ReplaceSubtree', { nodeId: 1, operations: [] }],
      ['BindAction', { nodeId: 1, event: EventId.TAP, action: 'server:increment_counter' }],
      ['UnbindAction', { nodeId: 1, event: EventId.TAP }],
      ['AttachState', { rootNode: 40, state: [...fixture('state_counter')] }],
      ['LiveMount', { nodeId: 1, channelId: 1n }],
      ['UiBehaviour', { nodeId: 1, behaviour: BehaviourId.DIALOG }],
    ]],
    ['web', WebOperation, [
      ['WebElement', { nodeId: 1, customTag: 'div' }],
      ['WebText', { nodeId: 1, content: 'a' }],
      ['WebSetAttribute', { nodeId: 1, customAttr: 'title', value: 'a' }],
      ['WebRemoveAttribute', { nodeId: 1, customAttr: 'title' }],
      ['WebProperty', { nodeId: 1, customName: 'title', value: 'a' }],
      ['WebAddListener', { nodeId: 1, customName: 'click' }],
      ['WebRemoveListener', { nodeId: 1, customName: 'click' }],
      ['WebReplaceNode', { oldId: 1, newId: 2 }],
      ['WebStyle', { nodeId: 1, customName: 'display', value: 'none' }],
      ['WebAddClass', { nodeId: 1, className: 'a' }],
      ['WebRemoveClass', { nodeId: 1, className: 'a' }],
      ['HtmlMorph', { target: { nodeId: 1 }, content: '<b>a</b>' }],
      ['WebRegisterNode', { nodeId: 1 }],
      ['WebUnregisterNode', { nodeId: 1 }],
      ['ScriptNode', { nodeId: 2, parentId: 1, body: 'void 0' }],
      ['WebBehaviour', { nodeId: 1, custom: 'a' }],
    ]],
    ['mobile', MobileOperation, [
      ['MountWebView', { nodeId: 2, parentId: 1, url: 'https://example.com' }],
      ['NativeCommand', { command: NativeCommandId.HAPTIC }],
      ['MobileBehaviour', { nodeId: 1, custom: 'a' }],
    ]],
  ];

  const { messages } = frameMessage('frame_ops_all_forms').unpack();
  let index = 0;
  for (const [group, union, forms] of groups) {
    for (const [name, fields] of forms) {
      const { operation } = messages[index++];
      for (const [kind, tags] of groups) {
        assert.equal(operation[`${kind}Type`], kind === group ? union[name] : tags.NONE, name);
      }
      assert.partialDeepStrictEqual(operation[group], fields, name);
    }
  }
  assert.equal(index, messages.length);
});

test('columnar text rows retain their values and start on an eight-byte boundary', () => {
  const batch = onlyMessage('frame_columnar_3rows').operation().columnar();
  const ids = batch.nodeIdsArray();
  assert.ok(ids instanceof Uint32Array);
  assert.equal(ids.byteOffset % 8, 0);
  assert.deepEqual([...ids], [1, 2, 3]);
  const value = batch.unpack();
  assert.deepEqual(value.opCodes, [UiOperation.SetText, UiOperation.SetText, UiOperation.SetText]);
  assert.deepEqual(value.argA, [0, 0, 0]);
  assert.deepEqual(value.argB, [0, 0, 0]);
  assert.deepEqual(value.blobAOff, [0, 1, 3, 6]);
  assert.deepEqual(value.blobBOff, [6, 6, 6, 6]);
  const data = new Uint8Array(value.blobBytes);
  const decoder = new TextDecoder();
  assert.deepEqual(
    value.blobAOff.slice(0, -1).map((start, row) => decoder.decode(data.subarray(start, value.blobAOff[row + 1]))),
    ['a', 'bb', 'ccc'],
  );
});

test('data table preserves integer and float columns', () => {
  const { table } = onlyMessage('frame_data_table').unpack().data;
  assert.equal(table.targetNode, 9);
  assert.equal(table.rows, 2n);
  assert.deepEqual(table.columns.map(({ name, dataType, data }) => [name, dataType, data.values]), [
    ['n', ColumnData.I64, [1n, 2n]],
    ['x', ColumnData.F64, [0.5, 1.5]],
  ]);
});

test('error control preserves its code, message, and first row index', () => {
  const { error } = onlyMessage('frame_error').unpack().control;
  assert.partialDeepStrictEqual(error, { code: ErrorCode.ActionFailed, message: 'failed', rowIndex: 0 });
});

test('expired state control preserves the issue time', () => {
  const { stateExpired } = onlyMessage('frame_state_expired').unpack().control;
  assert.equal(stateExpired.tokenIssuedAt, 1_700_000_000n);
});

test('the unknown operation keeps its raw tag and has no decoded value', () => {
  const operation = onlyMessage('frame_unknown_op').operation();
  assert.equal(operation.uiType(), 250);
  assert.equal(operation.unpack().ui, null);
});

test('action request carries the original state buffer', () => {
  const value = frame('action_increment').unpack();
  assert.equal(value.kindType, FrameKind.FrameRequest);
  assert.equal(value.streamId, 7n);
  assert.equal(value.sequenceId, 5n);
  const { action } = value.kind;
  assert.equal(action.action, 'server:increment_counter');
  assert.equal(action.nodeId, 42);
  assert.equal(action.input.eventName, 'tap');
  assert.deepEqual(action.state, [...fixture('state_counter')]);
});

test('web hello requests a new stream', () => {
  const value = frame('hello_web').unpack();
  assert.equal(value.kindType, FrameKind.FrameRequest);
  assert.equal(value.streamId, 0n);
  assert.equal(value.sequenceId, 1n);
  assert.partialDeepStrictEqual(value.kind.hello, {
    vocabularyVersion: 2,
    platform: Platform.Web,
    mount: MountSurface.Web,
    resumeFrom: 0n,
  });
});

test('server capabilities contain the action name and exact 64-bit hash', () => {
  const { caps } = onlyMessage('caps_actions').unpack().control;
  assert.equal(caps.status, CapsStatus.Ok);
  assert.equal(caps.schemaVersion, 1);
  assert.equal(caps.vocabularyVersion, 2);
  assert.equal(caps.actions.length, 1);
  assert.partialDeepStrictEqual(caps.actions[0], {
    hash: 0x0123_4567_89ab_cdefn,
    name: 'increment_counter',
  });
});

test('custom signer bytes travel unchanged on both attachments and the triggering node request', () => {
  const bytes = [...readFileSync(new URL('../../fixtures/state_counter_custom.json', import.meta.url))];
  const { messages } = frameMessage('frame_state_custom').unpack();
  assert.deepEqual(messages.map(({ operation }) => operation.uiType), [UiOperation.AttachState, UiOperation.AttachState]);
  assert.deepEqual(messages.map(({ operation }) => operation.ui.rootNode), [40, 42]);
  for (const { operation } of messages) assert.deepEqual(operation.ui.state, bytes);
  const { action } = frame('action_increment_custom').unpack().kind;
  assert.equal(action.nodeId, 42);
  assert.deepEqual(action.state, bytes);
});

test('signed state preserves header, session binding, body, and tag', () => {
  const buffer = new ByteBuffer(fixture('state_counter'));
  assert.ok(SignedState.bufferHasIdentifier(buffer));
  const state = SignedState.getRootAsSignedState(buffer).unpack();
  assert.partialDeepStrictEqual(state, {
    formatVersion: 1,
    flags: 0,
    keyId: 1,
    issuedAt: 1000n,
    expiresAt: 87400n,
    componentType: 7,
    rootNode: 40,
    sessionBinding: Array(32).fill(0x11),
    body: [1, 0, 0, 0],
    tag: Array(32).fill(0x22),
  });
});
