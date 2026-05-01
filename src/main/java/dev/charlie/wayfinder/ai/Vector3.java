package dev.charlie.wayfinder.ai;

public record Vector3(double x, double y, double z) {
    public double distanceTo(Vector3 other) {
        double dx = x - other.x;
        double dy = y - other.y;
        double dz = z - other.z;
        return Math.sqrt(dx * dx + dy * dy + dz * dz);
    }
}

