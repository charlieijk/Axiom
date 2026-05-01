package dev.charlie.wayfinder;

import com.mojang.brigadier.CommandDispatcher;
import dev.charlie.wayfinder.entity.WayfinderEntities;
import dev.charlie.wayfinder.entity.WayfinderVillagerEntity;
import java.util.Comparator;
import java.util.List;
import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;
import net.minecraft.server.command.ServerCommandSource;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.text.Text;
import net.minecraft.util.math.Box;

import static net.minecraft.server.command.CommandManager.literal;

public final class WayfinderCommands {
    private static final double COMMAND_SEARCH_RADIUS = 128.0;

    private WayfinderCommands() {
    }

    public static void register() {
        CommandRegistrationCallback.EVENT.register((dispatcher, registryAccess, environment) -> register(dispatcher));
    }

    private static void register(CommandDispatcher<ServerCommandSource> dispatcher) {
        dispatcher.register(literal("wayfinder")
                .then(literal("summon").executes(context -> summon(context.getSource())))
                .then(literal("recall").executes(context -> recall(context.getSource())))
                .then(literal("dismiss").executes(context -> dismiss(context.getSource())))
                .then(literal("debug")
                        .then(literal("on").executes(context -> setDebug(context.getSource(), true)))
                        .then(literal("off").executes(context -> setDebug(context.getSource(), false))))
                .then(literal("explain").executes(context -> explain(context.getSource()))));
    }

    private static int summon(ServerCommandSource source) {
        ServerPlayerEntity player = source.getPlayer();
        ServerWorld world = player.getServerWorld();

        WayfinderVillagerEntity entity = new WayfinderVillagerEntity(WayfinderEntities.WAYFINDER_VILLAGER, world);
        entity.refreshPositionAndAngles(player.getX() + 1.5, player.getY(), player.getZ() + 1.5, player.getYaw(), 0.0F);
        entity.setOwner(player);
        entity.setCustomName(Text.literal("Wayfinder"));
        entity.setPersistent();

        world.spawnEntity(entity);
        source.sendFeedback(() -> Text.literal("Summoned a Wayfinder Villager bound to " + player.getName().getString() + "."), false);
        return 1;
    }

    private static int recall(ServerCommandSource source) {
        ServerPlayerEntity player = source.getPlayer();
        WayfinderVillagerEntity companion = findNearestOwnedCompanion(player);
        if (companion == null) {
            source.sendError(Text.literal("No owned Wayfinder Villager found within " + (int) COMMAND_SEARCH_RADIUS + " blocks."));
            return 0;
        }

        companion.refreshPositionAndAngles(player.getX() + 1.5, player.getY(), player.getZ() + 1.5, player.getYaw(), 0.0F);
        companion.getNavigation().stop();
        source.sendFeedback(() -> Text.literal("Recalled your nearest Wayfinder Villager."), false);
        return 1;
    }

    private static int dismiss(ServerCommandSource source) {
        ServerPlayerEntity player = source.getPlayer();
        List<WayfinderVillagerEntity> companions = findOwnedCompanions(player);
        if (companions.isEmpty()) {
            source.sendError(Text.literal("No owned Wayfinder Villager found within " + (int) COMMAND_SEARCH_RADIUS + " blocks."));
            return 0;
        }

        companions.forEach(WayfinderVillagerEntity::discard);
        source.sendFeedback(() -> Text.literal("Dismissed " + companions.size() + " Wayfinder Villager(s)."), false);
        return companions.size();
    }

    private static int setDebug(ServerCommandSource source, boolean enabled) {
        ServerPlayerEntity player = source.getPlayer();
        List<WayfinderVillagerEntity> companions = findOwnedCompanions(player);
        if (companions.isEmpty()) {
            source.sendError(Text.literal("No owned Wayfinder Villager found within " + (int) COMMAND_SEARCH_RADIUS + " blocks."));
            return 0;
        }

        companions.forEach(companion -> companion.setDebugChatEnabled(enabled));
        source.sendFeedback(() -> Text.literal("Wayfinder reasoning chat " + (enabled ? "enabled" : "disabled") + "."), false);
        return companions.size();
    }

    private static int explain(ServerCommandSource source) {
        ServerPlayerEntity player = source.getPlayer();
        WayfinderVillagerEntity companion = findNearestOwnedCompanion(player);
        if (companion == null) {
            source.sendError(Text.literal("No owned Wayfinder Villager found within " + (int) COMMAND_SEARCH_RADIUS + " blocks."));
            return 0;
        }

        player.sendMessage(Text.literal(companion.describeLatestDecision()), false);
        return 1;
    }

    private static WayfinderVillagerEntity findNearestOwnedCompanion(ServerPlayerEntity player) {
        return findOwnedCompanions(player).stream()
                .min(Comparator.comparingDouble(companion -> companion.squaredDistanceTo(player)))
                .orElse(null);
    }

    private static List<WayfinderVillagerEntity> findOwnedCompanions(ServerPlayerEntity player) {
        Box searchBox = player.getBoundingBox().expand(COMMAND_SEARCH_RADIUS);
        return player.getServerWorld().getEntitiesByClass(
                WayfinderVillagerEntity.class,
                searchBox,
                companion -> companion.isOwner(player)
        );
    }
}
