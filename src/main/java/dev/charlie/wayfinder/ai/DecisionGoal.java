package dev.charlie.wayfinder.ai;

public enum DecisionGoal {
    STAY_ALIVE("Stay Alive"),
    FOLLOW_PLAYER("Follow Player"),
    MAINTAIN_POSITION("Maintain Position");

    private final String displayName;

    DecisionGoal(String displayName) {
        this.displayName = displayName;
    }

    public String displayName() {
        return displayName;
    }
}

