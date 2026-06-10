// A tiny DOM stand-in — just enough surface for ArrowDomApplicator, so DOM tests run
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
  }

  setAttribute(name, value) { this.attributes.set(name, value); }
  getAttribute(name) { return this.attributes.has(name) ? this.attributes.get(name) : null; }
  removeAttribute(name) { this.attributes.delete(name); }

  appendChild(child) {
    child.parent = this;
    this.children.push(child);
    return child;
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
  createElement(tag) { return new MockNode(tag); }
  createTextNode(text) {
    const n = new MockNode("#text");
    n.textContent = text;
    return n;
  }
}
