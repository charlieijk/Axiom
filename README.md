# Wayfinder AI

Wayfinder AI is a Fabric mod for Minecraft 1.21.1 that adds an explainable AI villager companion.

The first entity is the Wayfinder Villager: a survival escort NPC that follows its owner, avoids hazards, retreats from hostile mobs, and explains its current decision through owner-only chat messages.

## Commands

- `/wayfinder summon` spawns a Wayfinder Villager and binds it to the executing player.
- `/wayfinder recall` moves the nearest owned Wayfinder Villager back beside you.
- `/wayfinder dismiss` removes all owned Wayfinder Villagers within command range.
- `/wayfinder debug on` enables automatic reasoning chat for the player's companion.
- `/wayfinder debug off` disables automatic reasoning chat.
- `/wayfinder explain` prints the companion's latest decision on demand.

## Behavior

The companion observes nearby hazards, hostile mobs, owner distance, health, and safe candidate positions. A pure Java decision engine scores candidate actions and returns the selected goal, action, target, score, and explanation.

V1 actions:

- `AVOID_HAZARD`
- `RETREAT_FROM_MOB`
- `FOLLOW_OWNER`
- `HOLD_POSITION`

## Build

This project targets Java 21.

```sh
export JAVA_HOME=$(/usr/libexec/java_home -v 21)
GRADLE_USER_HOME=/tmp/wayfinder-gradle-home gradle test
GRADLE_USER_HOME=/tmp/wayfinder-gradle-home gradle build
```

The remapped mod jar is written to `build/libs/wayfinder-ai-0.1.0.jar`.

## Manual Test Loop

1. Install `build/libs/wayfinder-ai-0.1.0.jar` into a Fabric 1.21.1 instance.
2. Run `/wayfinder summon`.
3. Walk away and confirm the companion follows.
4. Place lava or fire nearby and confirm it moves away with a reason in chat.
5. Spawn a hostile mob nearby and confirm it retreats.
6. Run `/wayfinder explain` to inspect confidence and top candidate scores.
7. Run `/wayfinder recall` to reset its position, or `/wayfinder dismiss` to clean up.
