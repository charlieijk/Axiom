// An articulated character rig for the field-lab creature.
//
// WHAT IS SIMULATED AND WHAT IS NOT
//
// Axiom evolves planar creatures. Every simulated quantity a frame carries is
// two-dimensional: `frame.bodies[i]` is `[x, y]`, joint endpoints are `[x, y]`,
// and `frame.tilt` is a rotation about z. The physics has no third axis, and
// the existing box renderer is honest about that by drawing everything at
// z = 0.
//
// This module gives the same creature a character body — a torso with volume,
// limbs on both sides, a head that looks where it is going. All of that reads
// as depth, and none of it is simulated. So the rule this module is built
// around:
//
//   The spine carries simulation. Depth carries nothing.
//
// Concretely: every segment's centre is placed at exactly the simulated
// `[x, y]`, on the z = 0 plane, unchanged. A limb pair is TWO COPIES OF ONE
// SIMULATED SEGMENT, mirrored to ±z. The pair is not two legs that were
// separately evolved — there is only one, and drawing it twice is a reading
// aid, not data. `describeFidelity()` returns that sentence so a surface
// showing this rig can print it rather than letting a viewer infer that the
// creature was evolved in 3D.
//
// Gait sway, breathing and head turn are likewise presentational: they are
// derived from `frame.time` and the segment's own actuator value, never fed
// back into anything, and never displace a segment from its simulated centre.

/** The z offset of a mirrored limb pair, as a multiple of segment depth. */
export const MIRROR_SPREAD = 0.62;

/** Peak gait sway in radians. Small on purpose: it must not read as data. */
export const SWAY_RADIANS = 0.22;

/** What this rig does and does not claim. Surfaces should print this. */
export function describeFidelity() {
  return (
    "The creature is evolved in two dimensions. Segment centres are drawn at " +
    "their simulated positions; limbs are mirrored to both sides and all " +
    "depth, sway and head motion are presentational, carrying no simulated " +
    "value."
  );
}

/**
 * The planar angle of a segment, in radians.
 *
 * The root has no parent to point away from, so it uses the simulated tilt.
 * Every other segment points from its parent's centre to its own — the same
 * rule the box renderer uses, kept identical so the two presentations never
 * disagree about where a limb is.
 */
export function segmentAngle(frame, body, index) {
  const node = body[index];
  const parent = node ? node.parent : null;
  if (parent === null || parent === undefined || !frame.bodies[parent]) {
    return frame.tilt ?? 0;
  }
  const centre = frame.bodies[index];
  const origin = frame.bodies[parent];
  return Math.atan2(centre[1] - origin[1], centre[0] - origin[0]);
}

/**
 * Gait phase for one limb of a mirrored pair, in radians.
 *
 * `side` is +1 or -1. The two sides run in antiphase, which is what makes a
 * walker read as walking rather than twitching; `index` staggers successive
 * segments so a chain does not pulse in unison. Amplitude follows the
 * segment's actuator strength, so a limb the controller barely drives barely
 * moves — the one place depth motion is allowed to reflect something real,
 * and it reflects actuation, not position.
 */
export function limbPhase(index, time, actuator, side) {
  const strength = Math.min(1, Math.max(0, actuator ?? 0));
  const stagger = index * 0.7;
  return Math.sin(time * 6 + stagger + (side > 0 ? 0 : Math.PI)) * SWAY_RADIANS * strength;
}

/**
 * Which way the creature is facing, from how the root is moving.
 *
 * Returns +1 (travelling right, or stationary) or -1. A character that keeps
 * facing right while walking left is the single thing that makes a rig read as
 * broken, and the sign of the root's x velocity is the whole of the answer for
 * a planar creature.
 */
export function headingFrom(frame, previousRootX) {
  if (previousRootX === null || previousRootX === undefined) {
    return 1;
  }
  const delta = frame.root[0] - previousRootX;
  if (Math.abs(delta) < 1e-4) {
    return 0; // no opinion; the caller keeps the heading it had
  }
  return delta > 0 ? 1 : -1;
}

/**
 * Torso dimensions for a segment, in world units.
 *
 * Width and height come straight from the evolved size so the silhouette stays
 * the creature's own. Depth is invented — it is what turns a rectangle into a
 * body — and is derived from height rather than chosen freely, so a long thin
 * segment stays long and thin instead of becoming a slab.
 */
export function torsoScale(node, isRoot) {
  const width = Math.max(0.34, node.size[0] * 1.25);
  const height = Math.max(0.22, node.size[1] * 1.25);
  const depth = Math.max(0.26, height * (isRoot ? 1.35 : 1.05));
  return { width, height, depth };
}

/** Vertical bob of the whole body, in world units. Breathing, nothing more. */
export function breathOffset(time) {
  return Math.sin(time * 2.1) * 0.018;
}

const GROUND_LIFT = 0.46; // matches the box renderer, so both sit on the terrain

/**
 * Build the character rig.
 *
 * Mirrors `createCreatureRenderer`'s contract — `rebuild(body)` then
 * `update(frame, body, deltaSeconds)` — so the two are interchangeable and a
 * surface can offer either without knowing which it holds.
 */
export function createCharacterRenderer(THREE, scene) {
  const group = new THREE.Group();
  scene.add(group);

  const torsoGeometry = new THREE.BoxGeometry(1, 1, 1);
  const limbGeometry = new THREE.CapsuleGeometry
    ? new THREE.CapsuleGeometry(0.5, 1, 4, 10)
    : new THREE.CylinderGeometry(0.5, 0.5, 1, 12);
  const headGeometry = new THREE.SphereGeometry(0.5, 20, 14);
  const eyeGeometry = new THREE.SphereGeometry(0.5, 10, 8);

  const torsoMaterial = new THREE.MeshStandardMaterial({
    color: "#edf5e9",
    emissive: "#3f6d46",
    emissiveIntensity: 0.16,
    roughness: 0.42,
    metalness: 0.18,
  });
  const limbMaterial = new THREE.MeshStandardMaterial({
    color: "#80b7ea",
    emissive: "#193754",
    emissiveIntensity: 0.2,
    roughness: 0.46,
    metalness: 0.24,
  });
  const headMaterial = new THREE.MeshStandardMaterial({
    color: "#f4f7ef",
    emissive: "#46705a",
    emissiveIntensity: 0.18,
    roughness: 0.36,
    metalness: 0.12,
  });
  const eyeMaterial = new THREE.MeshStandardMaterial({
    color: "#16211f",
    emissive: "#8eea9f",
    emissiveIntensity: 0.55,
    roughness: 0.2,
    metalness: 0.1,
  });

  /** One entry per body segment: a torso, and the mirrored limb pair. */
  const segments = [];
  let head = null;
  let eye = null;
  let heading = 1;
  let previousRootX = null;

  const zAxis = new THREE.Vector3(0, 0, 1);
  const scratch = new THREE.Vector3();

  function clear() {
    for (const segment of segments) {
      group.remove(segment.torso);
      for (const limb of segment.limbs) {
        group.remove(limb.mesh);
      }
    }
    segments.length = 0;
    if (head) {
      group.remove(head);
      head = null;
    }
    if (eye) {
      group.remove(eye);
      eye = null;
    }
    heading = 1;
    previousRootX = null;
  }

  function rebuild(body) {
    clear();
    for (let index = 0; index < body.length; index += 1) {
      const torso = new THREE.Mesh(torsoGeometry, torsoMaterial);
      torso.castShadow = true;
      torso.receiveShadow = true;
      group.add(torso);

      // Two meshes, one simulated segment. See the note at the top of the file.
      const limbs = [1, -1].map((side) => {
        const mesh = new THREE.Mesh(limbGeometry, limbMaterial);
        mesh.castShadow = true;
        group.add(mesh);
        return { mesh, side };
      });

      segments.push({ torso, limbs });
    }

    head = new THREE.Mesh(headGeometry, headMaterial);
    head.castShadow = true;
    group.add(head);
    eye = new THREE.Mesh(eyeGeometry, eyeMaterial);
    group.add(eye);
  }

  function update(frame, body, deltaSeconds) {
    if (segments.length !== body.length) {
      rebuild(body);
    }
    const time = frame.time ?? 0;
    const bob = breathOffset(time);

    const nextHeading = headingFrom(frame, previousRootX);
    if (nextHeading !== 0) {
      heading = nextHeading;
    }
    previousRootX = frame.root[0];

    for (let index = 0; index < body.length; index += 1) {
      const node = body[index];
      const centre = frame.bodies[index];
      const angle = segmentAngle(frame, body, index);
      const { width, height, depth } = torsoScale(node, index === 0);
      const segment = segments[index];

      // The simulated position, unchanged, on the plane the physics ran in.
      segment.torso.position.set(centre[0], centre[1] + GROUND_LIFT + bob, 0);
      segment.torso.rotation.set(0, 0, angle);
      segment.torso.scale.set(width, height, depth);

      for (const limb of segment.limbs) {
        const swing = limbPhase(index, time, node.actuator, limb.side);
        // Offset along the segment's own z, then rotated with it, so a limb
        // pair stays attached to a tilted torso instead of shearing off it.
        scratch.set(0, -height * 0.45, limb.side * depth * MIRROR_SPREAD);
        scratch.applyAxisAngle(zAxis, angle);
        limb.mesh.position.set(
          centre[0] + scratch.x,
          centre[1] + GROUND_LIFT + bob + scratch.y,
          scratch.z,
        );
        limb.mesh.rotation.set(swing, 0, angle);
        limb.mesh.scale.set(depth * 0.34, height * 0.9, depth * 0.34);
        limb.mesh.visible = (node.actuator ?? 0) > 0.01;
      }
    }

    if (head && segments.length > 0) {
      const rootNode = body[0];
      const rootCentre = frame.bodies[0];
      const { width, height, depth } = torsoScale(rootNode, true);
      const reach = width * 0.5 * heading;
      head.position.set(
        rootCentre[0] + reach,
        rootCentre[1] + GROUND_LIFT + bob + height * 0.32,
        0,
      );
      head.scale.setScalar(Math.max(0.2, depth * 0.62));
      // A slow look-around, damped so it settles rather than oscillating.
      const look = Math.sin(time * 0.9) * 0.14;
      head.rotation.set(0, look, frame.tilt ?? 0);

      if (eye) {
        eye.visible = true;
        eye.position.set(
          head.position.x + reach * 0.55,
          head.position.y + depth * 0.1,
          depth * 0.22,
        );
        eye.scale.setScalar(Math.max(0.05, depth * 0.13));
      }
    }

    // Whole-body settle. Deliberately tiny, and applied to the group rather
    // than to any segment, so no simulated centre is ever displaced by it.
    if (deltaSeconds > 0) {
      group.rotation.y = Math.sin(time * 0.7) * 0.02;
    }
  }

  function dispose() {
    clear();
    scene.remove(group);
    torsoGeometry.dispose?.();
    limbGeometry.dispose?.();
    headGeometry.dispose?.();
    eyeGeometry.dispose?.();
  }

  return { rebuild, update, dispose, describeFidelity };
}
