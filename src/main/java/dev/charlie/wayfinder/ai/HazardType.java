package dev.charlie.wayfinder.ai;

public enum HazardType {
    LAVA("Lava", 5.0),
    FIRE("Fire", 4.0);

    private final String displayName;
    private final double severity;

    HazardType(String displayName, double severity) {
        this.displayName = displayName;
        this.severity = severity;
    }

    public String displayName() {
        return displayName;
    }

    public double severity() {
        return severity;
    }
}

