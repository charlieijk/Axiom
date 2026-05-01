package dev.charlie.wayfinder.entity;

import dev.charlie.wayfinder.ai.CandidatePosition;
import dev.charlie.wayfinder.ai.DecisionAction;
import dev.charlie.wayfinder.ai.DecisionEngine;
import dev.charlie.wayfinder.ai.DecisionResult;
import dev.charlie.wayfinder.ai.HazardType;
import dev.charlie.wayfinder.ai.ObservedHazard;
import dev.charlie.wayfinder.ai.ObservedThreat;
import dev.charlie.wayfinder.ai.Vector3;
import dev.charlie.wayfinder.ai.WorldSnapshot;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.UUID;
import net.minecraft.block.BlockState;
import net.minecraft.block.Blocks;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.ai.pathing.EntityNavigation;
import net.minecraft.entity.attribute.DefaultAttributeContainer;
import net.minecraft.entity.attribute.EntityAttributes;
import net.minecraft.entity.mob.HostileEntity;
import net.minecraft.entity.mob.PathAwareEntity;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Box;
import net.minecraft.util.math.Vec3d;
import net.minecraft.world.World;

public class WayfinderVillagerEntity extends PathAwareEntity {
    private static final int DECISION_INTERVAL_TICKS = 10;
    private static final int CHAT_INTERVAL_TICKS = 100;
    private static final int HAZARD_SCAN_RADIUS = 4;
    private static final int SAFE_SCAN_RADIUS = 6;
    private static final double THREAT_SCAN_RADIUS = 10.0;
    private static final double FOLLOW_SPEED = 1.05;
    private static final double SURVIVAL_SPEED = 1.18;

    private final DecisionEngine decisionEngine = new DecisionEngine();
    private UUID ownerUuid;
    private boolean debugChatEnabled = true;
    private DecisionResult latestDecision;
    private DecisionAction lastAnnouncedAction;
    private int lastChatAge = -CHAT_INTERVAL_TICKS;

    public WayfinderVillagerEntity(EntityType<? extends PathAwareEntity> entityType, World world) {
        super(entityType, world);
    }

    public static DefaultAttributeContainer.Builder createWayfinderAttributes() {
        return PathAwareEntity.createMobAttributes()
                .add(EntityAttributes.GENERIC_MAX_HEALTH, 24.0)
                .add(EntityAttributes.GENERIC_MOVEMENT_SPEED, 0.32)
                .add(EntityAttributes.GENERIC_FOLLOW_RANGE, 32.0);
    }

    @Override
    public void tick() {
        super.tick();

        if (!getWorld().isClient && age % DECISION_INTERVAL_TICKS == 0) {
            tickDecision();
        }
    }

    public void setOwner(ServerPlayerEntity owner) {
        ownerUuid = owner.getUuid();
    }

    public boolean isOwner(ServerPlayerEntity player) {
        return ownerUuid != null && ownerUuid.equals(player.getUuid());
    }

    public void setDebugChatEnabled(boolean debugChatEnabled) {
        this.debugChatEnabled = debugChatEnabled;
    }

    public String describeLatestDecision() {
        if (latestDecision == null) {
            return "[Wayfinder] No decision has been made yet.";
        }
        return latestDecision.detailedLine();
    }

    @Override
    public void writeCustomDataToNbt(NbtCompound nbt) {
        super.writeCustomDataToNbt(nbt);
        if (ownerUuid != null) {
            nbt.putUuid("Owner", ownerUuid);
        }
        nbt.putBoolean("DebugChat", debugChatEnabled);
    }

    @Override
    public void readCustomDataFromNbt(NbtCompound nbt) {
        super.readCustomDataFromNbt(nbt);
        if (nbt.containsUuid("Owner")) {
            ownerUuid = nbt.getUuid("Owner");
        }
        debugChatEnabled = !nbt.contains("DebugChat") || nbt.getBoolean("DebugChat");
    }

    private void tickDecision() {
        ServerPlayerEntity owner = getOwnerPlayer();
        if (owner == null || owner.isRemoved() || owner.isSpectator()) {
            getNavigation().stop();
            return;
        }

        WorldSnapshot snapshot = observe(owner);
        DecisionResult decision = decisionEngine.decide(snapshot);
        latestDecision = decision;

        executeDecision(decision, owner);
        maybeSendDecisionChat(decision, owner);
    }

    private ServerPlayerEntity getOwnerPlayer() {
        if (ownerUuid == null || !(getWorld() instanceof ServerWorld serverWorld)) {
            return null;
        }
        return serverWorld.getServer().getPlayerManager().getPlayer(ownerUuid);
    }

    private void executeDecision(DecisionResult decision, ServerPlayerEntity owner) {
        EntityNavigation navigation = getNavigation();
        if (decision.action() == DecisionAction.HOLD_POSITION) {
            navigation.stop();
            return;
        }

        if (decision.action() == DecisionAction.FOLLOW_OWNER) {
            navigation.startMovingTo(owner, FOLLOW_SPEED);
            return;
        }

        Vector3 target = decision.target();
        navigation.startMovingTo(target.x(), target.y(), target.z(), SURVIVAL_SPEED);
    }

    private void maybeSendDecisionChat(DecisionResult decision, ServerPlayerEntity owner) {
        if (!debugChatEnabled) {
            return;
        }

        boolean actionChanged = decision.action() != lastAnnouncedAction;
        boolean intervalElapsed = age - lastChatAge >= CHAT_INTERVAL_TICKS;
        if (actionChanged || intervalElapsed) {
            owner.sendMessage(Text.literal(decision.chatLine()), false);
            lastAnnouncedAction = decision.action();
            lastChatAge = age;
        }
    }

    private WorldSnapshot observe(ServerPlayerEntity owner) {
        Vector3 selfPosition = vectorFrom(getPos());
        Vector3 ownerPosition = vectorFrom(owner.getPos());
        List<ObservedHazard> hazards = scanHazards();
        List<ObservedThreat> threats = scanThreats();
        List<CandidatePosition> safePositions = scanSafePositions(ownerPosition, hazards, threats);

        return new WorldSnapshot(
                true,
                selfPosition,
                ownerPosition,
                selfPosition.distanceTo(ownerPosition),
                getHealth(),
                getMaxHealth(),
                hazards,
                threats,
                safePositions
        );
    }

    private List<ObservedHazard> scanHazards() {
        List<ObservedHazard> hazards = new ArrayList<>();
        BlockPos center = getBlockPos();

        for (int dx = -HAZARD_SCAN_RADIUS; dx <= HAZARD_SCAN_RADIUS; dx++) {
            for (int dy = -2; dy <= 2; dy++) {
                for (int dz = -HAZARD_SCAN_RADIUS; dz <= HAZARD_SCAN_RADIUS; dz++) {
                    BlockPos pos = center.add(dx, dy, dz);
                    HazardType hazardType = hazardTypeAt(pos);
                    if (hazardType != null) {
                        Vector3 hazardPosition = vectorFrom(pos);
                        hazards.add(new ObservedHazard(hazardType, hazardPosition, vectorFrom(getPos()).distanceTo(hazardPosition)));
                    }
                }
            }
        }

        return hazards.stream()
                .sorted(Comparator.comparingDouble(ObservedHazard::distance))
                .limit(8)
                .toList();
    }

    private List<ObservedThreat> scanThreats() {
        Box searchBox = getBoundingBox().expand(THREAT_SCAN_RADIUS);
        List<HostileEntity> hostiles = getWorld().getEntitiesByClass(HostileEntity.class, searchBox, LivingEntity::isAlive);
        Vector3 selfPosition = vectorFrom(getPos());

        return hostiles.stream()
                .map(hostile -> {
                    Vector3 threatPosition = vectorFrom(hostile.getPos());
                    return new ObservedThreat(hostile.getType().getName().getString(), threatPosition, selfPosition.distanceTo(threatPosition));
                })
                .sorted(Comparator.comparingDouble(ObservedThreat::distance))
                .limit(6)
                .toList();
    }

    private List<CandidatePosition> scanSafePositions(Vector3 ownerPosition, List<ObservedHazard> hazards, List<ObservedThreat> threats) {
        List<CandidatePosition> candidates = new ArrayList<>();
        BlockPos center = getBlockPos();
        Vector3 selfPosition = vectorFrom(getPos());

        for (int dx = -SAFE_SCAN_RADIUS; dx <= SAFE_SCAN_RADIUS; dx++) {
            for (int dy = -2; dy <= 2; dy++) {
                for (int dz = -SAFE_SCAN_RADIUS; dz <= SAFE_SCAN_RADIUS; dz++) {
                    BlockPos standPos = center.add(dx, dy, dz);
                    if (!isSafeStandPosition(standPos)) {
                        continue;
                    }

                    Vector3 position = vectorFrom(standPos);
                    candidates.add(new CandidatePosition(
                            position,
                            selfPosition.distanceTo(position),
                            ownerPosition.distanceTo(position),
                            nearestHazardDistance(position, hazards),
                            nearestThreatDistance(position, threats)
                    ));
                }
            }
        }

        return candidates.stream()
                .sorted(Comparator.comparingDouble(CandidatePosition::distanceToSelf))
                .limit(48)
                .toList();
    }

    private boolean isSafeStandPosition(BlockPos pos) {
        World world = getWorld();
        BlockPos belowPos = pos.down();
        BlockState below = world.getBlockState(belowPos);
        BlockState feet = world.getBlockState(pos);
        BlockState head = world.getBlockState(pos.up());

        boolean solidFloor = !below.getCollisionShape(world, belowPos).isEmpty();
        boolean feetClear = feet.getCollisionShape(world, pos).isEmpty();
        boolean headClear = head.getCollisionShape(world, pos.up()).isEmpty();

        return solidFloor
                && feetClear
                && headClear
                && hazardTypeAt(belowPos) == null
                && hazardTypeAt(pos) == null
                && hazardTypeAt(pos.up()) == null;
    }

    private HazardType hazardTypeAt(BlockPos pos) {
        BlockState state = getWorld().getBlockState(pos);
        if (state.isOf(Blocks.LAVA)) {
            return HazardType.LAVA;
        }
        if (state.isOf(Blocks.FIRE) || state.isOf(Blocks.SOUL_FIRE)) {
            return HazardType.FIRE;
        }
        return null;
    }

    private static double nearestHazardDistance(Vector3 position, List<ObservedHazard> hazards) {
        return hazards.stream()
                .mapToDouble(hazard -> hazard.position().distanceTo(position))
                .min()
                .orElse(12.0);
    }

    private static double nearestThreatDistance(Vector3 position, List<ObservedThreat> threats) {
        return threats.stream()
                .mapToDouble(threat -> threat.position().distanceTo(position))
                .min()
                .orElse(12.0);
    }

    private static Vector3 vectorFrom(Vec3d pos) {
        return new Vector3(pos.x, pos.y, pos.z);
    }

    private static Vector3 vectorFrom(BlockPos pos) {
        return new Vector3(pos.getX() + 0.5, pos.getY(), pos.getZ() + 0.5);
    }
}
