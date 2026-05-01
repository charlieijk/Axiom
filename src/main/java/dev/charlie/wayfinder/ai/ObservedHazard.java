package dev.charlie.wayfinder.ai;

public record ObservedHazard(HazardType type, Vector3 position, double distance) {
    public String displayName() {
        return type.displayName();
    }

    public double severity() {
        return type.severity();
    }
}

