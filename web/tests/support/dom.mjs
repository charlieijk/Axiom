// Minimal DOM stand-ins so the browser modules can be exercised under `node
// --test` without a headless browser or a DOM library. These deliberately model
// only what the code under test touches; anything else should fail loudly
// rather than silently pretend to work.

/**
 * A class list backed by a real Set, so `contains` reports what `add`/`remove`/
 * `toggle` actually did. The 3D journal's own status snapshot reads
 * `loadingState.classList.contains("is-visible")`, so a no-op stand-in here
 * would make that snapshot untestable.
 */
function createClassList() {
  const classes = new Set();
  return {
    classes,
    add(...names) {
      for (const name of names) {
        classes.add(name);
      }
    },
    remove(...names) {
      for (const name of names) {
        classes.delete(name);
      }
    },
    contains(name) {
      return classes.has(name);
    },
    toggle(name, force) {
      const next = force === undefined ? !classes.has(name) : Boolean(force);
      if (next) {
        classes.add(name);
      } else {
        classes.delete(name);
      }
      return next;
    },
  };
}

/**
 * Selector support is intentionally tiny: a bare tag name, matched against this
 * element's descendants. That is the whole vocabulary the page uses
 * (`button.querySelector("span")`, `generationRail.querySelectorAll("button")`).
 * Anything richer throws rather than quietly matching nothing.
 */
function matchDescendants(element, selector) {
  if (!/^[a-z]+$/.test(selector)) {
    throw new Error(`dom stub only supports bare tag selectors, got "${selector}"`);
  }
  const found = [];
  const visit = (node) => {
    for (const child of node.children) {
      if (child.tagName === selector) {
        found.push(child);
      }
      visit(child);
    }
  };
  visit(element);
  return found;
}

/** A stub element that records the writes the app makes to it. */
export function createElement(id = "", tagName = "div") {
  const listeners = new Map();
  const children = [];
  const attributes = new Map();
  return {
    id,
    tagName,
    value: "",
    textContent: "",
    className: "",
    type: "",
    disabled: false,
    max: "",
    dataset: {},
    style: {
      properties: new Map(),
      setProperty(name, propertyValue) {
        this.properties.set(name, propertyValue);
      },
      getPropertyValue(name) {
        return this.properties.get(name) ?? "";
      },
    },
    clientWidth: 960,
    clientHeight: 540,
    width: 960,
    height: 540,
    classList: createClassList(),
    listeners,
    children,
    get childElementCount() {
      return children.length;
    },
    addEventListener(type, handler) {
      const existing = listeners.get(type) ?? [];
      existing.push(handler);
      listeners.set(type, existing);
    },
    removeEventListener() {},
    /** Invoke the handlers the app registered, the way a real event would. */
    dispatch(type, event = {}) {
      for (const handler of listeners.get(type) ?? []) {
        handler(event);
      }
    },
    getBoundingClientRect() {
      return { width: this.clientWidth, height: this.clientHeight, top: 0, left: 0 };
    },
    getContext() {
      return createCanvasContext();
    },
    appendChild(child) {
      children.push(child);
      return child;
    },
    /** Both the element and the document-fragment spellings the journal uses. */
    replaceChildren(...nodes) {
      children.length = 0;
      for (const node of nodes) {
        if (node && Array.isArray(node.children)) {
          children.push(...node.children);
        } else if (node) {
          children.push(node);
        }
      }
    },
    setAttribute(name, attributeValue) {
      attributes.set(name, String(attributeValue));
    },
    getAttribute(name) {
      return attributes.has(name) ? attributes.get(name) : null;
    },
    querySelector(selector) {
      return matchDescendants(this, selector)[0] ?? null;
    },
    querySelectorAll(selector) {
      return matchDescendants(this, selector);
    },
    // Pointer capture is a no-op here; the page only needs the calls to succeed.
    setPointerCapture() {},
    releasePointerCapture() {},
  };
}

// The 2D drawing calls are not what these tests are about: every method is a
// no-op and every assigned property (fillStyle, lineWidth, ...) just sticks.
// Calls return one shared result object so the shapes the app actually chains
// off a return value keep working -- gradients (`.addColorStop`) and text
// measurement (`.width`).
export function createCanvasContext() {
  const callResult = {
    addColorStop() {},
    width: 0,
  };
  return new Proxy(
    {},
    {
      get(target, property) {
        if (property in target) {
          return target[property];
        }
        return () => callResult;
      },
    },
  );
}

/**
 * A window stand-in. `reducedMotion` drives the
 * `prefers-reduced-motion: reduce` media query the pages consult before they
 * decide to autoplay.
 */
export function createWindowStub({ reducedMotion = false } = {}) {
  const listeners = new Map();
  const timeouts = [];
  return {
    devicePixelRatio: 1,
    innerWidth: 1280,
    innerHeight: 720,
    listeners,
    /** Callbacks handed to setTimeout, so a test can run them on demand. */
    timeouts,
    matchMedia(query) {
      return {
        media: query,
        matches: query.includes("prefers-reduced-motion") ? reducedMotion : false,
        addEventListener() {},
        removeEventListener() {},
        addListener() {},
        removeListener() {},
      };
    },
    addEventListener(type, handler) {
      const existing = listeners.get(type) ?? [];
      existing.push(handler);
      listeners.set(type, existing);
    },
    removeEventListener() {},
    dispatch(type, event = {}) {
      for (const handler of listeners.get(type) ?? []) {
        handler(event);
      }
    },
    setTimeout(handler) {
      timeouts.push(handler);
      return timeouts.length;
    },
    clearTimeout() {},
    requestAnimationFrame() {
      return 0;
    },
    cancelAnimationFrame() {},
  };
}

export function createDocumentStub() {
  const elements = new Map();
  return {
    elements,
    getElementById(id) {
      if (!elements.has(id)) {
        elements.set(id, createElement(id));
      }
      return elements.get(id);
    },
    createElement(tagName = "div") {
      return createElement("", tagName);
    },
    // A fragment is just a parentless element here: the journal only appends to
    // it and then hands it to replaceChildren.
    createDocumentFragment() {
      return createElement("", "#document-fragment");
    },
    addEventListener() {},
    querySelectorAll() {
      return [];
    },
  };
}
