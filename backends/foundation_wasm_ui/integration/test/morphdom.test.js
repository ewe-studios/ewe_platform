// Feature 07 — MorphDom reconciliation on the mock DOM. Covers the spec's
// core table: text/attr updates, identity preservation, ID-anchored reorders
// and moves, tag-mismatch/duplicate ID exclusion, pantry park+retrieve, the
// prepend anti-churn rule, the displacement limit, form-state preservation,
// and the data-ignore-morph / data-preserve-attr escape hatches.
// (Script execution tests are browser-only — executeNewScripts guards on
// querySelectorAll and is exercised in the web testbed.)

import { test } from "node:test";
import assert from "node:assert/strict";

import { MorphDom } from "../../runtimes/foundation-wasm-ui.js";
import { MockDocument } from "../mock-dom.js";

const doc = () => new MockDocument();

/** Build an element with attrs + children in one call. */
function el(d, tag, attrs = {}, children = []) {
  const node = d.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  for (const child of children) {
    node.appendChild(typeof child === "string" ? d.createTextNode(child) : child);
  }
  return node;
}

function text(node) {
  return node.children.map((c) => (c.tag === "#text" ? c.textContent : text(c))).join("");
}

/// Test 1 — text updates reuse the element.
test("text change reuses the element", () => {
  const d = doc();
  const target = el(d, "div", {}, [el(d, "p", {}, ["old"])]);
  const p = target.children[0];
  MorphDom.morph(target, el(d, "div", {}, [el(d, "p", {}, ["new"])]), d);
  assert.equal(target.children[0], p, "same <p>");
  assert.equal(text(target), "new");
});

/// Tests 2 + 3 — child add/remove leaves siblings untouched.
test("add and remove children preserve siblings", () => {
  const d = doc();
  const target = el(d, "ul", {}, [el(d, "li", {}, ["a"])]);
  const a = target.children[0];

  MorphDom.morph(target, el(d, "ul", {}, [el(d, "li", {}, ["a"]), el(d, "li", {}, ["b"])]), d);
  assert.equal(target.children.length, 2);
  assert.equal(target.children[0], a, "existing li unchanged");

  MorphDom.morph(target, el(d, "ul", {}, [el(d, "li", {}, ["a"])]), d);
  assert.equal(target.children.length, 1);
  assert.equal(target.children[0], a);
});

/// Test 4 — attribute sync on the same element.
test("attribute sync updates and removes", () => {
  const d = doc();
  const target = el(d, "div", {}, [el(d, "span", { class: "a", title: "t" })]);
  const span = target.children[0];
  MorphDom.morph(target, el(d, "div", {}, [el(d, "span", { class: "b" })]), d);
  assert.equal(target.children[0], span);
  assert.equal(span.getAttribute("class"), "b");
  assert.equal(span.getAttribute("title"), null, "absent attr removed");
});

/// Test 7 — ID reorder: same three elements, zero creation.
test("id-anchored reorder moves, never recreates", () => {
  const d = doc();
  const target = el(d, "div", {}, [
    el(d, "p", { id: "a" }, ["A"]),
    el(d, "p", { id: "b" }, ["B"]),
    el(d, "p", { id: "c" }, ["C"]),
  ]);
  const [a, b, c] = target.children;
  MorphDom.morph(
    target,
    el(d, "div", {}, [
      el(d, "p", { id: "c" }, ["C"]),
      el(d, "p", { id: "a" }, ["A"]),
      el(d, "p", { id: "b" }, ["B"]),
    ]),
    d,
  );
  assert.deepEqual(target.children, [c, a, b], "identical elements, new order");
});

/// Test 8 — a moved node keeps its subtree identity.
test("moved subtree keeps inner identity", () => {
  const d = doc();
  const inner = el(d, "span", {}, ["deep"]);
  const target = el(d, "div", {}, [
    el(d, "section", { id: "s" }, [inner]),
    el(d, "p", { id: "p" }, ["after"]),
  ]);
  const section = target.children[0];
  MorphDom.morph(
    target,
    el(d, "div", {}, [
      el(d, "p", { id: "p" }, ["after"]),
      el(d, "section", { id: "s" }, [el(d, "span", {}, ["deep"])]),
    ]),
    d,
  );
  assert.equal(target.children[1], section, "section moved, same element");
  assert.equal(section.children[0], inner, "child survived the move");
});

/// Test 9 — id on a DIFFERENT tag is excluded from persistence.
test("id tag mismatch falls back to create", () => {
  const d = doc();
  const target = el(d, "div", {}, [el(d, "div", { id: "x" })]);
  const oldDiv = target.children[0];
  MorphDom.morph(target, el(d, "div", {}, [el(d, "span", { id: "x" })]), d);
  assert.notEqual(target.children[0], oldDiv, "div removed");
  assert.equal(target.children[0].tag, "span", "span created fresh");
});

/// Test 10 — duplicate ids in the old tree are excluded.
test("duplicate old ids are not persistent", () => {
  const d = doc();
  const target = el(d, "div", {}, [
    el(d, "p", { id: "dup" }, ["one"]),
    el(d, "p", { id: "dup" }, ["two"]),
  ]);
  // Must not throw and must end with exactly the new content.
  MorphDom.morph(target, el(d, "div", {}, [el(d, "p", { id: "dup" }, ["only"])]), d);
  assert.equal(target.children.length, 1);
  assert.equal(text(target), "only");
});

/// Test 11 — pantry: parked early, retrieved later IN the same morph.
test("pantry parks and retrieves within one morph", () => {
  const d = doc();
  const target = el(d, "div", {}, [
    el(d, "p", { id: "keep" }, ["KEEP"]),
    el(d, "span", { id: "tail" }, ["TAIL"]),
  ]);
  const keep = target.children[0];
  // New order: tail first, keep second — keep gets parked while tail matches,
  // then retrieved from the pantry.
  MorphDom.morph(
    target,
    el(d, "div", {}, [
      el(d, "span", { id: "tail" }, ["TAIL"]),
      el(d, "p", { id: "keep" }, ["KEEP"]),
    ]),
    d,
  );
  assert.equal(target.children[1], keep, "same element back from the pantry");
  assert.equal(keep.parent, target, "no longer parked");
});

/// Test 13 — prepend anti-churn: B/C/D stay themselves.
test("prepend without ids does not churn", () => {
  const d = doc();
  const target = el(d, "ul", {}, [
    el(d, "li", {}, ["B"]),
    el(d, "li", {}, ["C"]),
    el(d, "li", {}, ["D"]),
  ]);
  const [b, c, dd] = target.children;
  MorphDom.morph(
    target,
    el(d, "ul", {}, [
      el(d, "li", {}, ["A"]),
      el(d, "li", {}, ["B"]),
      el(d, "li", {}, ["C"]),
      el(d, "li", {}, ["D"]),
    ]),
    d,
  );
  assert.equal(target.children.length, 4);
  assert.deepEqual(target.children.slice(1), [b, c, dd], "B/C/D kept their identity");
  assert.equal(text(target), "ABCD");
});

/// Test 14 — prepend WITH ids: anchored matches, one creation.
test("prepend with ids matches by id", () => {
  const d = doc();
  const target = el(d, "ul", {}, [
    el(d, "li", { id: "b" }, ["B"]),
    el(d, "li", { id: "c" }, ["C"]),
  ]);
  const [b, c] = target.children;
  MorphDom.morph(
    target,
    el(d, "ul", {}, [
      el(d, "li", { id: "a" }, ["A"]),
      el(d, "li", { id: "b" }, ["B"]),
      el(d, "li", { id: "c" }, ["C"]),
    ]),
    d,
  );
  assert.equal(target.children[1], b);
  assert.equal(target.children[2], c);
});

/// Test 15 — displacement limit: don't drag past anchored nodes.
test("displacement limit blocks long pulls", () => {
  const d = doc();
  const target = el(d, "div", {}, [
    el(d, "p", { id: "x" }, ["X"]),
    el(d, "p", { id: "y" }, ["Y"]),
    el(d, "p", { id: "z" }, ["Z"]),
    el(d, "p", {}, ["W"]),
  ]);
  const w = target.children[3];
  MorphDom.morph(
    target,
    el(d, "div", {}, [el(d, "p", {}, ["W"]), el(d, "p", { id: "x" }, ["X"])]),
    d,
  );
  assert.notEqual(target.children[0], w, "W was created fresh, not pulled past 3 anchors");
  assert.equal(text(target), "WX");
});

/// Test 16 — 100-item list, first moved to last: 100 survivors.
test("large list move keeps identity", () => {
  const d = doc();
  const items = Array.from({ length: 100 }, (_, i) => el(d, "li", { id: `i${i}` }, [`${i}`]));
  const target = el(d, "ul", {}, items);
  const order = [...items.slice(1), items[0]];
  MorphDom.morph(
    target,
    el(d, "ul", {}, order.map((n) => el(d, "li", { id: n.getAttribute("id") }, [text({ children: n.children })]))),
    d,
  );
  assert.equal(target.children.length, 100);
  for (let i = 0; i < 100; i++) {
    assert.equal(target.children[i], order[i], `position ${i} is the original element`);
  }
});

/// Tests 17-21 + 23 — form state preservation.
test("form state survives morphs", () => {
  const d = doc();
  const input = el(d, "input", { type: "text", placeholder: "old" });
  input.value = "hello"; // user typed
  const box = el(d, "input", { type: "checkbox" });
  box.checked = true; // user checked
  const area = el(d, "textarea", {});
  area.value = "edited";
  const select = el(d, "select", {}, [el(d, "option"), el(d, "option")]);
  select.selectedIndex = 1;

  const target = el(d, "form", {}, [input, box, area, select]);
  MorphDom.morph(
    target,
    el(d, "form", {}, [
      el(d, "input", { type: "text", placeholder: "new" }),
      el(d, "input", { type: "checkbox" }),
      el(d, "textarea", {}),
      el(d, "select", {}, [el(d, "option"), el(d, "option")]),
    ]),
    d,
  );
  assert.equal(target.children[0], input, "input reused");
  assert.equal(input.getAttribute("placeholder"), "new", "attrs synced");
  assert.equal(input.value, "hello", "user value preserved");
  assert.equal(box.checked, true, "checkbox state preserved");
  assert.equal(area.value, "edited");
  assert.equal(select.selectedIndex, 1);
});

/// Tests 27 + 28 — data-ignore-morph.
test("data-ignore-morph requires both sides", () => {
  const d = doc();
  const widget = el(d, "div", { "data-ignore-morph": "", class: "untouched" }, ["WIDGET"]);
  const target = el(d, "section", {}, [widget]);

  // Both sides marked: untouched.
  MorphDom.morph(
    target,
    el(d, "section", {}, [el(d, "div", { "data-ignore-morph": "", class: "changed" })]),
    d,
  );
  assert.equal(widget.getAttribute("class"), "untouched");
  assert.equal(text(target), "WIDGET");

  // Only old side marked: morphs normally.
  MorphDom.morph(target, el(d, "section", {}, [el(d, "div", { class: "changed" })]), d);
  assert.equal(widget.getAttribute("class"), "changed");
});

/// Test 29 — data-preserve-attr shields named attributes.
test("data-preserve-attr keeps listed attrs", () => {
  const d = doc();
  const node = el(d, "div", {
    "data-preserve-attr": "style,class",
    style: "color:red",
    class: "mine",
    title: "old",
  });
  const target = el(d, "section", {}, [node]);
  MorphDom.morph(
    target,
    el(d, "section", {}, [el(d, "div", { style: "color:blue", class: "theirs", title: "new" })]),
    d,
  );
  assert.equal(node.getAttribute("style"), "color:red", "preserved");
  assert.equal(node.getAttribute("class"), "mine", "preserved");
  assert.equal(node.getAttribute("title"), "new", "others synced");
});

/// Test 31 — empty new content removes all children.
test("empty content clears the target", () => {
  const d = doc();
  const target = el(d, "div", {}, [el(d, "p", {}, ["x"]), el(d, "p", {}, ["y"])]);
  MorphDom.morph(target, el(d, "div"), d);
  assert.equal(target.children.length, 0);
});
