package dev.charlie.wayfinder.entity;

import dev.charlie.wayfinder.WayfinderAiMod;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;

public final class WayfinderEntities {
    public static final EntityType<WayfinderVillagerEntity> WAYFINDER_VILLAGER = Registry.register(
            Registries.ENTITY_TYPE,
            WayfinderAiMod.id("wayfinder_villager"),
            EntityType.Builder.create(WayfinderVillagerEntity::new, SpawnGroup.CREATURE)
                    .dimensions(0.6F, 1.95F)
                    .maxTrackingRange(10)
                    .build("wayfinder_ai:wayfinder_villager")
    );

    private WayfinderEntities() {
    }

    public static void register() {
        FabricDefaultAttributeRegistry.register(WAYFINDER_VILLAGER, WayfinderVillagerEntity.createWayfinderAttributes());
    }
}

