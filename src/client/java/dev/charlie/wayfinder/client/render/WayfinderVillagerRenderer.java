package dev.charlie.wayfinder.client.render;

import dev.charlie.wayfinder.WayfinderAiMod;
import dev.charlie.wayfinder.entity.WayfinderVillagerEntity;
import net.minecraft.client.render.entity.EntityRendererFactory;
import net.minecraft.client.render.entity.MobEntityRenderer;
import net.minecraft.client.render.entity.model.EntityModelLayers;
import net.minecraft.client.render.entity.model.VillagerResemblingModel;
import net.minecraft.util.Identifier;

public class WayfinderVillagerRenderer extends MobEntityRenderer<WayfinderVillagerEntity, VillagerResemblingModel<WayfinderVillagerEntity>> {
    private static final Identifier TEXTURE = WayfinderAiMod.id("textures/entity/wayfinder_villager.png");

    public WayfinderVillagerRenderer(EntityRendererFactory.Context context) {
        super(context, new VillagerResemblingModel<>(context.getPart(EntityModelLayers.VILLAGER)), 0.5F);
    }

    @Override
    public Identifier getTexture(WayfinderVillagerEntity entity) {
        return TEXTURE;
    }
}

