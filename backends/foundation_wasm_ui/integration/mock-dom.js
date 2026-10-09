// A tiny DOM stand-in — just enough surface for DomOpApplicator, so DOM tests run
// under plain `node --test` without jsdom.

class ClassList {
  constructor() { this._set = new Set(); }
  add(c) { this._set.add(c); }
  remove(c) { this._set.delete(c); }
  contains(c) { return this._set.has(c); }
  get length() { return this._set.size; }
}

export class MockNode {
  constructor(tag = "#text") {
    this.tag = tag;
    this.className = "";
    this.textContent = "";
    this.innerHTML = "";
    this.children = [];
    this.parent = null;
    this.removed = false;
    this.attributes = new Map();
    this.classList = new ClassList();
    this.style = {};
    // form-ish props the EventDispatcher reads into EventData
    this.value = null;
    this.checked = null;
    this._listeners = new Map(); // type -> Set<fn>
  }

  setAttribute(name, value) { this.attributes.set(name, value); }
  getAttribute(name) { return this.attributes.has(name) ? this.attributes.get(name) : null; }
  removeAttribute(name) { this.attributes.delete(name); }
  getAttributeNames() { return [...this.attributes.keys()]; }

  addEventListener(type, fn) {
    let set = this._listeners.get(type);
    if (!set) { set = new Set(); this._listeners.set(type, set); }
    set.add(fn);
  }
  removeEventListener(type, fn) {
    this._listeners.get(type)?.delete(fn);
  }
  /** Fire an event of `type`; `init` supplies keyCode/modifier fields. */
  dispatchEvent(type, init = {}) {
    const event = { type, target: this, ...init };
    for (const fn of this._listeners.get(type) ?? []) fn(event);
  }
  /** Count of listeners for `type` — lets tests assert idempotent wiring. */
  listenerCount(type) {
    return this._listeners.get(type)?.size ?? 0;
  }

  appendChild(child) {
    child.parent = this;
    this.children.push(child);
    return child;
  }

  replaceChild(newChild, oldChild) {
    const i = this.children.indexOf(oldChild);
    if (i >= 0) {
      this.children[i] = newChild;
      newChild.parent = this;
      oldChild.parent = null;
    }
    return oldChild;
  }

  removeChild(child) {
    const i = this.children.indexOf(child);
    if (i >= 0) {
      this.children.splice(i, 1);
      child.parent = null;
    }
    return child;
  }

  /** DOM-alias so code written against `parentNode` works on mocks too. */
  get parentNode() {
    return this.parent;
  }

  /** Sibling/child traversal aliases for the morph engine. */
  get childNodes() {
    return this.children;
  }

  get firstChild() {
    return this.children[0] ?? null;
  }

  get nextSibling() {
    if (!this.parent) return null;
    const i = this.parent.children.indexOf(this);
    return i >= 0 ? this.parent.children[i + 1] ?? null : null;
  }

  /** Structural equality (DOM's isEqualNode, mock edition). */
  isEqualNode(other) {
    if (!other || other.tag !== this.tag) return false;
    if (this.tag === "#text") return this.textContent === other.textContent;
    const mine = this.getAttributeNames().sort();
    const theirs = (other.getAttributeNames?.() ?? []).sort();
    if (mine.length !== theirs.length) return false;
    for (let i = 0; i < mine.length; i++) {
      if (mine[i] !== theirs[i]) return false;
      if (this.getAttribute(mine[i]) !== other.getAttribute(mine[i])) return false;
    }
    if (this.children.length !== other.children.length) return false;
    return this.children.every((child, i) => child.isEqualNode(other.children[i]));
  }

  /** DOM-alias for the event runtime's delegate resolution. */
  get parentElement() {
    return this.parent;
  }

  /** Subtree containment (delegation guards). */
  contains(node) {
    for (let cur = node; cur; cur = cur.parent) {
      if (cur === this) return true;
    }
    return false;
  }

  /** Nearest ancestor-or-self matching a bare tag name (island boundary). */
  closest(tag) {
    for (let cur = this; cur; cur = cur.parent) {
      if (cur.tag === tag) return cur;
    }
    return null;
  }

  /**
   * Records adjacent-HTML insertions for assertions (real parsing/morphing is
   * feature 07); `position` is one of beforebegin/afterbegin/beforeend/afterend.
   */
  insertAdjacentHTML(position, html) {
    (this.adjacentHTML ??= []).push({ position, html });
  }

  insertBefore(child, ref) {
    const i = this.children.indexOf(ref);
    child.parent = this;
    if (i < 0) this.children.push(child);
    else this.children.splice(i, 0, child);
    return child;
  }

  remove() {
    this.removed = true;
    if (this.parent) {
      const i = this.parent.children.indexOf(this);
      if (i >= 0) this.parent.children.splice(i, 1);
      this.parent = null;
    }
  }

  replaceWith(node) {
    if (this.parent) {
      const i = this.parent.children.indexOf(this);
      if (i >= 0) this.parent.children[i] = node;
      node.parent = this.parent;
      this.parent = null;
    }
    this.removed = true;
  }
}

export class MockDocument {
  constructor() {
    // Root for querySelector walks; tests append what they want findable.
    this.root = new MockNode("#document");
  }

  createElement(tag) { return new MockNode(tag); }
  createTextNode(text) {
    const n = new MockNode("#text");
    n.textContent = text;
    return n;
  }

  /**
   * Minimal selector engine over `root`: `[primal-id="N"]`, `#id`, `.class`,
   * or a bare tag name — exactly what REGISTER_NODE and MORPH_NODE use.
   */
  querySelector(selector) {
    const matches = (node) => {
      const attr = /^\[primal-id="(.+)"\]$/.exec(selector);
      if (attr) return String(node.getAttribute("primal-id")) === attr[1];
      if (selector.startsWith("#")) return node.getAttribute("id") === selector.slice(1);
      if (selector.startsWith(".")) return node.classList.contains(selector.slice(1));
      return node.tag === selector;
    };
    const walk = (node) => {
      if (matches(node)) return node;
      for (const child of node.children) {
        const hit = walk(child);
        if (hit) return hit;
      }
      return null;
    };
    return walk(this.root);
  }
}
