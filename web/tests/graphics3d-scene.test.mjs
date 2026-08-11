// The 3D scene builders take THREE as a parameter rather than importing it, so
// they can be exercised against a recording stand-in. These tests cover the
// geometry maths and the camera framing -- the parts that are the module's own
// logic rather than three.js behaviour.
import assert from "node:assert/strict";
import test from "node:test";

import { createDocumentStub, createWindowStub } from "./support/dom.mjs";

globalThis.document = createDocumentStub();
globalThis.window = createWindowStub();

const { createTerrain, updateCamera } = await import("../graphics3d-scene.js");
const { state } = await import("../graphics3d-state.js");

/** The slice of three.js the terrain builder touches. */
function fakeThree() {
  class Float32BufferAttribute {
    constructor(array, itemSize) {
      this.array = array;
      this.itemSize = itemSize;
    }
  }
  class BufferGeometry {
    constructor() {
      this.attributes = {};
      this.index = null;
      this.normalsComputed = false;
      this.disposed = false;
    }
    setAttribute(name, attribute) {
      this.attributes[name] = attribute;
    }
    setIndex(index) {
      this.index = index;
    }
    computeVertexNormals() {
      this.normalsComputed = true;
    }
    dispose() {
      this.disposed = true;
    }
  }
  class MeshStandardMaterial {
    constructor(options) {
      Object.assign(this, options);
      this.disposed = false;
    }
    dispose() {
      this.disposed = true;
    }
  }
  class Mesh {
    constructor(geometry, material) {
      this.geometry = geometry;
      this.material = material;
    }
  }
  return { Float32BufferAttribute, BufferGeometry, MeshStandardMaterial, Mesh };
}

function heightsOf(terrain) {
  const positions = terrain.mesh.geometry.attributes.position.array;
  const heights = [];
  for (let index = 1; index < positions.length; index += 3) {
    heights.push(positions[index]);
  }
  return heights;
}

test("createTerrain builds a fully indexed grid with normals", () => {
  const terrain = createTerrain(fakeThree(), "rough");
  const geometry = terrain.mesh.geometry;

  // 240x28 quads means 241x29 vertices, each contributing x/y/z.
  assert.equal(geometry.attributes.position.array.length, 241 * 29 * 3);
  assert.equal(geometry.attributes.position.itemSize, 3);
  // Two triangles per quad, three indices each.
  assert.equal(geometry.index.length, 240 * 28 * 6);
  assert.ok(geometry.normalsComputed, "lighting needs vertex normals");

  const highestIndex = Math.max(...geometry.index);
  assert.ok(
    highestIndex < geometry.attributes.position.array.length / 3,
    "every index must reference a vertex that exists",
  );
});

test("the flat task removes the ridge roughness but keeps the cross ripple", () => {
  const rough = heightsOf(createTerrain(fakeThree(), "rough"));
  const flat = heightsOf(createTerrain(fakeThree(), "flat"));

  const spread = (values) => Math.max(...values) - Math.min(...values);

  assert.ok(
    spread(flat) < spread(rough),
    `flat terrain should be calmer than rough (flat ${spread(flat)}, rough ${spread(rough)})`,
  );
  // "flat" names the task, not a plane: a shallow ripple across z remains.
  assert.ok(spread(flat) > 0, "flat terrain still carries its cross-axis ripple");
});

test("createTerrain hands back a disposer that releases geometry and material", () => {
  const terrain = createTerrain(fakeThree(), "rough");
  terrain.dispose();

  assert.ok(terrain.mesh.geometry.disposed, "geometry must be released");
  assert.ok(terrain.mesh.material.disposed, "material must be released");
});

/** A minimal vector/camera pair matching the three.js surface updateCamera uses. */
function makeVector(x = 0, y = 0, z = 0) {
  return {
    x,
    y,
    z,
    set(nx, ny, nz) {
      this.x = nx;
      this.y = ny;
      this.z = nz;
      return this;
    },
    copy(other) {
      this.x = other.x;
      this.y = other.y;
      this.z = other.z;
      return this;
    },
    add(other) {
      this.x += other.x;
      this.y += other.y;
      this.z += other.z;
      return this;
    },
    lerp(target, alpha) {
      this.x += (target.x - this.x) * alpha;
      this.y += (target.y - this.y) * alpha;
      this.z += (target.z - this.z) * alpha;
      return this;
    },
  };
}

function makeCamera(aspect = 1.6) {
  const looked = [];
  return {
    aspect,
    position: makeVector(0, 0, 0),
    looked,
    lookAt(target) {
      looked.push({ x: target.x, y: target.y, z: target.z });
    },
  };
}

function makeScratch() {
  return { target: makeVector(), offset: makeVector(), desired: makeVector() };
}

const frame = { time: 0, root: [4, 0.2] };

test("updateCamera keeps the look-at target above the terrain", () => {
  const camera = makeCamera();
  const scratch = makeScratch();

  // A root well below the ground: the framing must lift to clear the terrain
  // rather than aiming the camera into it.
  updateCamera(camera, { time: 0, root: [4, -5] }, "rough", 1 / 60, scratch);

  const target = camera.looked.at(-1);
  assert.ok(target.y > -5, "target must not follow the body below the terrain");
  assert.equal(scratch.target.z, 0, "the journal frames the run on the z=0 plane");
});

test("updateCamera leads ahead of the body in follow mode", () => {
  const camera = makeCamera();
  const scratch = makeScratch();
  state.cameraMode = "follow";

  updateCamera(camera, frame, "rough", 1 / 60, scratch);

  assert.ok(
    scratch.target.x > frame.root[0],
    "follow framing looks ahead of the creature, not straight at it",
  );
});

test("orbit mode removes the sideways offset that follow mode uses", () => {
  const scratch = makeScratch();
  state.cameraMode = "follow";
  updateCamera(makeCamera(), frame, "rough", 1 / 60, scratch);
  const followOffsetX = scratch.offset.x;

  const orbitScratch = makeScratch();
  state.cameraMode = "orbit";
  state.orbitYaw = 0;
  updateCamera(makeCamera(), frame, "rough", 1 / 60, orbitScratch);

  assert.notEqual(followOffsetX, orbitScratch.offset.x);
  // With no yaw, orbit framing sits directly behind the subject.
  assert.equal(orbitScratch.offset.x, 0);

  state.cameraMode = "follow";
});

test("a larger frame delta moves the camera further toward its target", () => {
  const scratch = makeScratch();
  state.cameraMode = "follow";

  const slow = makeCamera();
  updateCamera(slow, frame, "rough", 1 / 240, scratch);

  const fast = makeCamera();
  updateCamera(fast, frame, "rough", 1 / 30, makeScratch());

  // Both start at the origin, so the one given more time must travel further.
  const distance = (camera) => Math.hypot(camera.position.x, camera.position.y, camera.position.z);
  assert.ok(
    distance(fast) > distance(slow),
    "camera smoothing must be frame-rate independent, not per-call",
  );
});
