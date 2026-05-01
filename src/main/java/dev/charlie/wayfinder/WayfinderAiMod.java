package dev.charlie.wayfinder;

import dev.charlie.wayfinder.entity.WayfinderEntities;
import net.fabricmc.api.ModInitializer;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class WayfinderAiMod implements ModInitializer {
    public static final String MOD_ID = "wayfinder_ai";
    public static final Logger LOGGER = LoggerFactory.getLogger(MOD_ID);

    @Override
    public void onInitialize() {
        WayfinderEntities.register();
        WayfinderCommands.register();
        LOGGER.info("Wayfinder AI initialized");
    }

    public static Identifier id(String path) {
        return Identifier.of(MOD_ID, path);
    }
}

