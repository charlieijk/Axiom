package dev.charlie.wayfinder.ai;

public enum DecisionAction {
    AVOID_HAZARD("Avoid Hazard"),
    RETREAT_FROM_MOB("Retreat From Mob"),
    FOLLOW_OWNER("Follow Owner"),
    HOLD_POSITION("Hold Position");

    private final String displayName;

    DecisionAction(String displayName) {
        this.displayName = displayName;
    }

    public String displayName() {
        return displayName;
    }
}

