// Minimal DOM stand-ins so the browser modules can be exercised under `node
// --test` without a headless browser or a DOM library. These deliberately model
// only what the code under test touches; anything else should fail loudly
// rather than silently pretend to work.

/** A stub element that records the writes the app makes to it. */
export function createElement(id = "") {
  const listeners = new Map();
  return {
    id,
    value: "",
    textContent: "",
    max: "",
    dataset: {},
    style: {},
    clientWidth: 960,
    clientHeight: 540,
    width: 960,
    height: 540,
    classList: { add() {}, remove() {}, toggle() {} },
    listeners,
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
    appendChild() {},
    setAttribute() {},
    querySelectorAll() {
      return [];
    },
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
 * `prefers-reduced-motion: reduce` media query the 3D page consults before it
 * decides to autoplay.
 */
export function createWindowStub({ reducedMotion = false } = {}) {
  return {
    devicePixelRatio: 1,
    innerWidth: 1280,
    innerHeight: 720,
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
    addEventListener() {},
    removeEventListener() {},
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
    createElement() {
      return createElement();
    },
    addEventListener() {},
    querySelectorAll() {
      return [];
    },
  };
}
