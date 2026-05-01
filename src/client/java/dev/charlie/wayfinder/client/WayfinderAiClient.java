package dev.charlie.wayfinder.client;

import dev.charlie.wayfinder.client.render.WayfinderVillagerRenderer;
import dev.charlie.wayfinder.entity.WayfinderEntities;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;

public class WayfinderAiClient implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        EntityRendererRegistry.register(WayfinderEntities.WAYFINDER_VILLAGER, WayfinderVillagerRenderer::new);
    }
}

