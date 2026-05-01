package dev.charlie.wayfinder.ai;

import java.util.List;

public record DecisionCandidate(
        DecisionGoal goal,
        DecisionAction action,
        double score,
        Vector3 target,
        List<ReasonFactor> reasonFactors
) {
    public DecisionCandidate {
        reasonFactors = List.copyOf(reasonFactors);
    }
}

